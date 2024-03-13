#![cfg(not(target_family = "wasm"))]

mod docs_generator;
mod jinja;
mod watch;

use anstream::{eprintln, println};
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Result;
use ariadne::Source;
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clio::has_extension;
use clio::Output;
use itertools::Itertools;
use std::collections::HashMap;
use std::env;
use std::io::{Read, Write};
use std::ops::Range;
use std::path::Path;
use std::process::exit;
use std::str::FromStr;

use prqlc::ast;
use prqlc::semantic;
use prqlc::semantic::reporting::{collect_frames, label_references};
use prqlc::semantic::NS_DEFAULT_DB;
use prqlc::{ir::pl::Lineage, ir::Span};
use prqlc::{pl_to_prql, pl_to_rq_tree, prql_to_pl, prql_to_pl_tree, rq_to_sql, SourceTree};
use prqlc::{Options, Target};

/// Entrypoint called by [`crate::main`]
pub fn main() -> color_eyre::eyre::Result<()> {
    env_logger::builder().format_timestamp(None).init();
    color_eyre::install()?;
    let mut cli = Cli::parse();
    cli.color.write_global();

    if let Err(error) = cli.command.run() {
        eprintln!("{error}");
        // Copied from
        // https://doc.rust-lang.org/src/std/backtrace.rs.html#1-504, since it's private
        fn backtrace_enabled() -> bool {
            match env::var("RUST_LIB_BACKTRACE") {
                Ok(s) => s != "0",
                Err(_) => match env::var("RUST_BACKTRACE") {
                    Ok(s) => s != "0",
                    Err(_) => false,
                },
            }
        }
        if backtrace_enabled() {
            eprintln!("{:#}", error.backtrace());
        }

        exit(1)
    }

    Ok(())
}

#[derive(Parser, Debug, Clone)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[command(flatten)]
    color: colorchoice_clap::Color,
}

#[derive(Subcommand, Debug, Clone)]
#[command(name = env!("CARGO_PKG_NAME"), about, version)]
enum Command {
    /// Parse into PL AST
    Parse {
        #[command(flatten)]
        io_args: IoArgs,
        #[arg(value_enum, long, default_value = "yaml")]
        format: Format,
    },

    /// Parse & generate PRQL code back
    #[command(name = "fmt")]
    Format {
        #[arg(value_parser, default_value = "-", value_hint(ValueHint::AnyPath))]
        input: clio::ClioPath,
    },

    /// Parse the whole project and collect it into a single PRQL source file
    #[command(name = "collect")]
    Collect(IoArgs),

    #[command(subcommand)]
    Debug(DebugCommand),

    #[command(subcommand)]
    Experimental(ExperimentalCommand),

    /// Parse, resolve & lower into RQ
    Resolve {
        #[command(flatten)]
        io_args: IoArgs,
        #[arg(value_enum, long, default_value = "yaml")]
        format: Format,
    },

    /// Parse, resolve, lower into RQ & preprocess SRQ
    #[command(name = "sql:preprocess")]
    SQLPreprocess(IoArgs),

    /// Parse, resolve, lower into RQ & preprocess & anchor SRQ
    ///
    /// Only displays the main pipeline.
    #[command(name = "sql:anchor")]
    SQLAnchor {
        #[command(flatten)]
        io_args: IoArgs,

        #[arg(value_enum, long, default_value = "yaml")]
        format: Format,
    },

    /// Parse, resolve, lower into RQ & compile to SQL
    ///
    /// Only displays the main pipeline and does not handle loop.
    #[command(name = "compile", alias = "sql:compile")]
    SQLCompile {
        #[command(flatten)]
        io_args: IoArgs,

        /// Exclude the signature comment containing the PRQL version
        #[arg(long = "hide-signature-comment", action = clap::ArgAction::SetFalse)]
        signature_comment: bool,

        /// Emit unformatted, dense SQL
        #[arg(long = "no-format", action = clap::ArgAction::SetFalse)]
        format: bool,

        /// Target to compile to
        #[arg(short, long, default_value = "sql.any", env = "PRQLC_TARGET")]
        target: String,
    },

    /// Watch a directory and compile .prql files to .sql files
    Watch(watch::WatchArgs),

    /// Show available compile target names
    #[command(name = "list-targets")]
    ListTargets,

    /// Print a shell completion for supported shells
    #[command(name = "shell-completion")]
    ShellCompletion {
        #[arg(value_enum)]
        shell: clap_complete_command::Shell,
    },
}

/// Commands for meant for debugging, prone to change
#[derive(Subcommand, Debug, Clone)]
pub enum DebugCommand {
    /// Parse & and expand into PL, but don't resolve
    ExpandPL(IoArgs),

    /// Parse & resolve, but don't lower into RQ
    Resolve(IoArgs),

    /// Parse & evaluate expression down to a value
    ///
    /// Cannot contain references to tables or any other outside sources.
    /// Meant as a playground for testing out language design decisions.
    Eval(IoArgs),

    /// Parse, resolve & combine source with comments annotating relation type
    Annotate(IoArgs),

    /// Print info about the AST data structure
    Ast,
}

/// Experimental commands are prone to change
#[derive(Subcommand, Debug, Clone)]
pub enum ExperimentalCommand {
    /// Generate Markdown documentation
    #[command(name = "doc")]
    GenerateDocs(IoArgs),
}

#[derive(clap::Args, Default, Debug, Clone)]
pub struct IoArgs {
    #[arg(value_parser, default_value = "-", value_hint(ValueHint::AnyPath))]
    input: clio::ClioPath,

    #[arg(value_parser, default_value = "-", value_hint(ValueHint::FilePath))]
    output: Output,

    /// Identifier of the main pipeline.
    #[arg(value_parser, value_hint(ValueHint::Unknown))]
    main_path: Option<String>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum Format {
    Json,
    Yaml,
}

impl Command {
    /// Entrypoint called by [`main`]
    pub fn run(&mut self) -> Result<()> {
        match self {
            Command::Watch(command) => watch::run(command),
            Command::ListTargets => self.list_targets(),
            // Format is handled differently to the other IO commands, since it
            // always writes to the same output.
            Command::Format { input } => {
                let sources = read_files(input)?;
                let root = sources.root;

                for (path, source) in sources.sources {
                    let ast = prql_to_pl(&source)?;

                    // If we're writing to stdout (though could this be nicer?
                    // We're discarding many of the benefits of Clio here...)
                    if path.as_os_str() == "" {
                        let mut output: Output = Output::new(input.path())?;
                        output.write_all(&pl_to_prql(&ast)?.into_bytes())?;
                        break;
                    }

                    let path_buf = root
                        .as_ref()
                        .map_or_else(|| path.clone(), |root| root.join(&path));
                    let path_str = path_buf.to_str().ok_or_else(|| {
                        anyhow!("Path `{}` is not valid UTF-8", path_buf.display())
                    })?;
                    let mut output: Output = Output::new(path_str)?;

                    output.write_all(&pl_to_prql(&ast)?.into_bytes())?;
                }
                Ok(())
            }
            Command::ShellCompletion { shell } => {
                shell.generate(&mut Cli::command(), &mut std::io::stdout());
                Ok(())
            }
            Command::Debug(DebugCommand::Ast) => {
                prqlc::ir::pl::print_mem_sizes();
                Ok(())
            }
            _ => self.run_io_command(),
        }
    }

    fn list_targets(&self) -> std::result::Result<(), anyhow::Error> {
        let res: Result<std::string::String, anyhow::Error> = Ok(match self {
            Command::ListTargets => Target::names().join("\n"),
            _ => unreachable!(),
        });

        match res {
            Ok(s) => println!("{s}"),
            Err(_) => unreachable!(),
        }

        Ok(())
    }

    fn run_io_command(&mut self) -> std::result::Result<(), anyhow::Error> {
        let (mut file_tree, main_path) = self.read_input()?;

        self.execute(&mut file_tree, &main_path)
            .and_then(|buf| Ok(self.write_output(&buf)?))
    }

    fn execute<'a>(&self, sources: &'a mut SourceTree, main_path: &'a str) -> Result<Vec<u8>> {
        let main_path = main_path
            .split('.')
            .filter(|x| !x.is_empty())
            .map(str::to_string)
            .collect_vec();

        Ok(match self {
            Command::Parse { format, .. } => {
                let ast = prql_to_pl_tree(sources)?;
                match format {
                    Format::Json => serde_json::to_string_pretty(&ast)?.into_bytes(),
                    Format::Yaml => serde_yaml::to_string(&ast)?.into_bytes(),
                }
            }
            Command::Collect(_) => {
                let mut root_module_def = prql_to_pl_tree(sources)?;

                drop_module_def(&mut root_module_def.stmts, "std");

                pl_to_prql(&root_module_def)?.into_bytes()
            }
            Command::Debug(DebugCommand::ExpandPL(_)) => {
                let root_module_def = prql_to_pl_tree(sources)?;

                let expanded = prqlc::semantic::ast_expand::expand_module_def(root_module_def)?;

                let mut restricted = prqlc::semantic::ast_expand::restrict_module_def(expanded);

                drop_module_def(&mut restricted.stmts, "std");

                pl_to_prql(&restricted)?.into_bytes()
            }
            Command::Debug(DebugCommand::Resolve(_)) => {
                let stmts = prql_to_pl_tree(sources)?;

                let root_module = semantic::resolve(stmts, Default::default())
                    .map_err(|e| prqlc::ErrorMessages::from(e).composed(sources))?;

                // debug output of the PL
                let mut out = format!("{root_module:#?}\n").into_bytes();

                // labelled sources
                for (source_id, source) in &sources.sources {
                    let source_id = source_id.to_str().unwrap().to_string();
                    out.extend(label_references(&root_module, source_id, source.clone()));
                }

                // resolved PL, restricted back into AST
                let mut root_module = semantic::ast_expand::restrict_module(root_module.module);
                drop_module_def(&mut root_module.stmts, "std");
                out.extend(pl_to_prql(&root_module)?.into_bytes());

                out
            }
            Command::Debug(DebugCommand::Annotate(_)) => {
                let (_, source) = sources.sources.clone().into_iter().exactly_one().or_else(
                    |_| bail!(
                        "Currently `annotate` only works with a single source, but found multiple sources: {:?}",
                        sources.sources.keys()
                            .map(|x| x.display().to_string())
                            .sorted()
                            .map(|x| format!("`{x}`"))
                            .join(", ")
                    )
                )?;

                // TODO: potentially if there is code performing a role beyond
                // presentation, it should be a library function; and we could
                // promote it to the `prqlc` crate.
                let root_mod = prql_to_pl(&source)?;

                // resolve
                let ctx = semantic::resolve(root_mod, Default::default())?;

                let frames = if let Ok((main, _)) = ctx.find_main_rel(&[]) {
                    collect_frames(*main.clone().into_relation_var().unwrap())
                } else {
                    vec![]
                };

                // combine with source
                combine_prql_and_frames(&source, frames).as_bytes().to_vec()
            }
            Command::Debug(DebugCommand::Eval(_)) => {
                let root_mod = prql_to_pl_tree(sources)?;

                let mut res = String::new();
                for stmt in root_mod.stmts {
                    if let ast::StmtKind::VarDef(def) = stmt.kind {
                        res += &format!("## {}\n", def.name);

                        let val = semantic::eval(*def.value.unwrap())
                            .map_err(|e| prqlc::ErrorMessages::from(e).composed(sources))?;
                        res += &semantic::write_pl(val);
                        res += "\n\n";
                    }
                }

                res.into_bytes()
            }
            Command::Experimental(ExperimentalCommand::GenerateDocs(_)) => {
                let module_ref = prql_to_pl_tree(sources)?;

                docs_generator::generate_markdown_docs(module_ref.stmts).into_bytes()
            }
            Command::Resolve { format, .. } => {
                let ast = prql_to_pl_tree(sources)?;
                let ir = pl_to_rq_tree(ast, &main_path, &[NS_DEFAULT_DB.to_string()])?;

                match format {
                    Format::Json => serde_json::to_string_pretty(&ir)?.into_bytes(),
                    Format::Yaml => serde_yaml::to_string(&ir)?.into_bytes(),
                }
            }
            Command::SQLCompile {
                signature_comment,
                format,
                target,
                ..
            } => {
                let opts = Options::default()
                    .with_target(Target::from_str(target).map_err(prqlc::ErrorMessages::from)?)
                    .with_signature_comment(*signature_comment)
                    .with_format(*format);

                prql_to_pl_tree(sources)
                    .and_then(|pl| pl_to_rq_tree(pl, &main_path, &[NS_DEFAULT_DB.to_string()]))
                    .and_then(|rq| rq_to_sql(rq, &opts))
                    .map_err(|e| e.composed(sources))?
                    .as_bytes()
                    .to_vec()
            }

            Command::SQLPreprocess { .. } => {
                let ast = prql_to_pl_tree(sources)?;
                let rq = pl_to_rq_tree(ast, &main_path, &[NS_DEFAULT_DB.to_string()])?;
                let srq = prqlc::sql::internal::preprocess(rq)?;
                format!("{srq:#?}").as_bytes().to_vec()
            }
            Command::SQLAnchor { format, .. } => {
                let ast = prql_to_pl_tree(sources)?;
                let rq = pl_to_rq_tree(ast, &main_path, &[NS_DEFAULT_DB.to_string()])?;
                let srq = prqlc::sql::internal::anchor(rq)?;

                let json = serde_json::to_string_pretty(&srq)?;

                match format {
                    Format::Json => json.into_bytes(),
                    Format::Yaml => {
                        let val: serde_yaml::Value = serde_yaml::from_str(&json)?;
                        serde_yaml::to_string(&val)?.into_bytes()
                    }
                }
            }

            _ => unreachable!(),
        })
    }

    fn read_input(&mut self) -> Result<(SourceTree, String)> {
        // Possibly this should be called by the relevant subcommands passing in
        // `input`, rather than matching on them and grabbing `input` from
        // `self`? But possibly if everything moves to `io_args`, then this is
        // quite reasonable?
        use Command::{
            Collect, Debug, Experimental, Parse, Resolve, SQLAnchor, SQLCompile, SQLPreprocess,
        };
        let io_args = match self {
            Parse { io_args, .. }
            | Collect(io_args)
            | Resolve { io_args, .. }
            | SQLCompile { io_args, .. }
            | SQLPreprocess(io_args)
            | SQLAnchor { io_args, .. }
            | Debug(
                DebugCommand::Resolve(io_args)
                | DebugCommand::ExpandPL(io_args)
                | DebugCommand::Annotate(io_args)
                | DebugCommand::Eval(io_args),
            ) => io_args,
            Experimental(ExperimentalCommand::GenerateDocs(io_args)) => io_args,
            _ => unreachable!(),
        };
        let input = &mut io_args.input;

        // Don't wait without a prompt when running `prqlc compile` —
        // it's confusing whether it's waiting for input or not. This
        // offers the prompt.
        //
        // See https://github.com/PRQL/prql/issues/3228 for details on us not
        // yet using `input.is_tty()`.
        // if input.is_tty() {
        if input.path() == Path::new("-") && atty::is(atty::Stream::Stdin) {
            #[cfg(unix)]
            eprintln!("Enter PRQL, then press ctrl-d to compile:\n");
            #[cfg(windows)]
            eprintln!("Enter PRQL, then press ctrl-z to compile:\n");
        }

        let sources = read_files(input)?;

        let main_path = io_args.main_path.clone().unwrap_or_default();

        Ok((sources, main_path))
    }

    fn write_output(&mut self, data: &[u8]) -> std::io::Result<()> {
        use Command::{
            Collect, Debug, Experimental, Parse, Resolve, SQLAnchor, SQLCompile, SQLPreprocess,
        };
        let mut output = match self {
            Parse { io_args, .. }
            | Collect(io_args)
            | Resolve { io_args, .. }
            | SQLCompile { io_args, .. }
            | SQLAnchor { io_args, .. }
            | SQLPreprocess(io_args)
            | Debug(
                DebugCommand::Resolve(io_args)
                | DebugCommand::ExpandPL(io_args)
                | DebugCommand::Annotate(io_args)
                | DebugCommand::Eval(io_args),
            ) => io_args.output.clone(),
            Experimental(ExperimentalCommand::GenerateDocs(io_args)) => io_args.output.clone(),
            _ => unreachable!(),
        };
        output.write_all(data)
    }
}

fn drop_module_def(stmts: &mut Vec<ast::stmt::Stmt>, name: &str) {
    stmts.retain(|x| x.kind.as_module_def().map_or(true, |m| m.name != name));
}

fn read_files(input: &mut clio::ClioPath) -> Result<SourceTree> {
    // Should this function move to a SourceTree constructor?
    let root = input.path();

    let mut sources = HashMap::new();
    for file in input.clone().files(has_extension("prql"))? {
        let path = file.path().strip_prefix(root)?.to_owned();

        let mut file_contents = String::new();
        file.open()?.read_to_string(&mut file_contents)?;

        sources.insert(path, file_contents);
    }
    Ok(SourceTree::new(sources, Some(root.to_path_buf())))
}

fn combine_prql_and_frames(source: &str, frames: Vec<(Span, Lineage)>) -> String {
    let source = Source::from(source);
    let lines = source.lines().collect_vec();
    let width = lines.iter().map(|l| l.len()).max().unwrap_or(0);

    // Not sure this is the nicest construction. Some of this was added in the
    // upgrade from ariande 0.3.0 to 0.4.0 and possibly there are more elegant
    // ways to build the output. (Though `.get_line_text` seems to be the only
    // method to get actual text out of a `Source`...)
    let mut printed_lines_count = 0;
    let mut result = Vec::new();
    for (span, frame) in frames {
        let line_len = source.get_line_range(&Range::from(span)).end - 1;

        while printed_lines_count < line_len {
            result.push(
                source
                    .get_line_text(source.line(printed_lines_count).unwrap())
                    .unwrap()
                    .to_string(),
            );
            printed_lines_count += 1;
        }

        if printed_lines_count >= lines.len() {
            break;
        }
        let chars: String = source
            .get_line_text(source.line(printed_lines_count).unwrap())
            .unwrap()
            .to_string();
        printed_lines_count += 1;

        result.push(format!("{chars:width$} # {frame}"));
    }
    for line in lines.iter().skip(printed_lines_count) {
        result.push(source.get_line_text(line.to_owned()).unwrap().to_string());
    }

    result.into_iter().join("\n") + "\n"
}

/// Unit tests for `prqlc`. Integration tests (where we call the actual binary)
/// are in `prqlc/tests/test.rs`.
#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use super::*;

    #[test]
    fn layouts() {
        let output = Command::execute(
            &Command::Debug(DebugCommand::Annotate(IoArgs::default())),
            &mut r#"
from db.initial_table
select {f = first_name, l = last_name, gender}
derive full_name = f"{f} {l}"
take 23
select {f"{l} {f}", full = full_name, gender}
sort full
        "#
            .into(),
            "",
        )
        .unwrap();
        assert_snapshot!(String::from_utf8(output).unwrap().trim(),
        @r###"
        from db.initial_table
        select {f = first_name, l = last_name, gender}  # [f, l, initial_table.gender]
        derive full_name = f"{f} {l}"                   # [f, l, initial_table.gender, full_name]
        take 23                                         # [f, l, initial_table.gender, full_name]
        select {f"{l} {f}", full = full_name, gender}   # [?, full, initial_table.gender]
        sort full                                       # [?, full, initial_table.gender]
        "###);
    }

    /// Check we get an error on a bad input
    #[test]
    fn compile() {
        anstream::ColorChoice::Never.write_global();

        let result = Command::execute(
            &Command::SQLCompile {
                io_args: IoArgs::default(),
                signature_comment: false,
                format: true,
                target: "sql.any".to_string(),
            },
            &mut "asdf".into(),
            "",
        );

        assert_snapshot!(&result.unwrap_err().to_string(), @r###"
        Error:
           ╭─[:1:1]
           │
         1 │ asdf
           │ ──┬─
           │   ╰─── Unknown name `asdf`
        ───╯
        "###);
    }

    #[test]
    fn compile_multiple() {
        let result = Command::execute(
            &Command::SQLCompile {
                io_args: IoArgs::default(),
                signature_comment: false,
                format: true,
                target: "sql.any".to_string(),
            },
            &mut SourceTree::new(
                [
                    ("Project.prql".into(), "orders.x | select y".to_string()),
                    (
                        "orders.prql".into(),
                        "let x = (from db.z | select {y, u})".to_string(),
                    ),
                ],
                None,
            ),
            "main",
        )
        .unwrap();
        assert_snapshot!(String::from_utf8(result).unwrap().trim(), @r###"
        WITH x AS (
          SELECT
            y,
            u
          FROM
            z
        )
        SELECT
          y
        FROM
          x
        "###);
    }

    #[test]
    fn parse() {
        let output = Command::execute(
            &Command::Parse {
                io_args: IoArgs::default(),
                format: Format::Yaml,
            },
            &mut "from x | select y".into(),
            "",
        )
        .unwrap();

        assert_snapshot!(String::from_utf8(output).unwrap().trim(), @r###"
        name: Project
        stmts:
        - VarDef:
            kind: Main
            name: main
            value:
              Pipeline:
                exprs:
                - FuncCall:
                    name:
                      Ident:
                      - from
                    args:
                    - Ident:
                      - x
                - FuncCall:
                    name:
                      Ident:
                      - select
                    args:
                    - Ident:
                      - y
          span: 1:0-17
        "###);
    }
    #[test]
    fn resolve() {
        let output = Command::execute(
            &Command::Resolve {
                io_args: IoArgs::default(),
                format: Format::Yaml,
            },
            &mut "from db.x | select y".into(),
            "",
        )
        .unwrap();

        assert_snapshot!(String::from_utf8(output).unwrap().trim(), @r###"
        def:
          version: null
          other: {}
        tables:
        - id: 0
          name: null
          relation:
            kind: !ExternRef
            - x
            columns:
            - !Single y
            - Wildcard
        relation:
          kind: !Pipeline
          - !From
            source: 0
            columns:
            - - !Single y
              - 0
            - - Wildcard
              - 1
            name: x
          - !Select
            - 0
          - !Select
            - 0
          columns:
          - !Single y
        "###);
    }

    #[test]
    fn sql_anchor() {
        let output = Command::execute(
            &Command::SQLAnchor {
                io_args: IoArgs::default(),
                format: Format::Yaml,
            },
            &mut "from db.employees | sort salary | take 3 | filter salary > 0".into(),
            "",
        )
        .unwrap();

        assert_snapshot!(String::from_utf8(output).unwrap().trim(), @r###"
        ctes:
        - tid: 1
          kind:
            Normal:
              AtomicPipeline:
              - Select:
                - 0
                - 1
              - From:
                  kind:
                    Ref: 0
                  riid: 0
              - Sort:
                - direction: Asc
                  column: 0
              - Take:
                  range:
                    start: null
                    end:
                      kind:
                        Literal:
                          Integer: 3
                      span: null
                  partition: []
                  sort:
                  - direction: Asc
                    column: 0
        main_relation:
          AtomicPipeline:
          - From:
              kind:
                Ref: 1
              riid: 1
          - Select:
            - 2
            - 3
          - Filter:
              kind:
                Operator:
                  name: std.gt
                  args:
                  - kind:
                      ColumnRef: 2
                    span: 1:50-56
                  - kind:
                      Literal:
                        Integer: 0
                    span: 1:59-60
              span: 1:50-60
          - Sort:
            - direction: Asc
              column: 2
        "###);
    }
}
