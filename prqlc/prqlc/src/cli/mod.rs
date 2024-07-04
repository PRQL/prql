#![cfg(not(target_family = "wasm"))]

mod docs_generator;
mod jinja;
mod watch;

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::str::FromStr;

use anstream::{eprintln, println};
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Result;
use ariadne::Source;
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_verbosity_flag::LogLevel;
use clio::has_extension;
use clio::Output;
use is_terminal::IsTerminal;
use itertools::Itertools;
use schemars::schema_for;

use prqlc::debug;
use prqlc::internal::pl_to_lineage;
use prqlc::ir::{pl, rq};
use prqlc::pr;
use prqlc::semantic;
use prqlc::semantic::reporting::FrameCollector;
use prqlc::{pl_to_prql, pl_to_rq_tree, prql_to_pl, prql_to_pl_tree, prql_to_tokens, rq_to_sql};
use prqlc::{Options, SourceTree, Target};

/// Entrypoint called by [`crate::main`]
pub fn main() -> color_eyre::eyre::Result<()> {
    let mut cli = Cli::parse();

    // redirect all log messages into the [debug::DebugLog]
    static LOGGER: debug::MessageLogger = debug::MessageLogger;
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(cli.verbose.log_level_filter()))
        .unwrap();

    color_eyre::install()?;
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

    #[command(flatten)]
    verbose: clap_verbosity_flag::Verbosity<LoggingHelp>,
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

    /// Lex into Lexer Representation
    Lex {
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

    /// Parse, resolve, lower into RQ & compile to SQL
    ///
    /// Only displays the main pipeline and does not handle loop.
    #[command(name = "compile")]
    Compile {
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

        /// File path into which to write the debug log to.
        #[arg(long, env = "PRQLC_DEBUG_LOG")]
        debug_log: Option<PathBuf>,
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
enum DebugCommand {
    /// Parse, resolve & combine source with comments annotating relation type
    Annotate(IoArgs),

    /// Output column-level lineage graph
    ///
    /// The returned data includes:
    ///
    /// * "frames": a list of Span and Lineage records corresponding to each
    ///   transformation frame in the main pipeline.
    ///
    /// * "nodes": a list of expression graph nodes.
    ///
    /// * "ast": the parsed PL abstract syntax tree.
    ///
    /// Each expression node has attributes:
    ///
    /// * "id": A unique ID for each expression.
    ///
    /// * "kind": Descriptive text about the expression type.
    ///
    /// * "span": Position of the expression in the original source (optional).
    ///
    /// * "alias": When this expression is part of a Tuple, this is its alias
    ///   (optional).
    ///
    /// * "ident": When this expression is an Ident, this is its reference
    ///   (optional).
    ///
    /// * "targets": Any upstream sources of data for this expression, as a list
    ///   of node IDs (optional).
    ///
    /// * "children": A list of expression IDs contained within this expression
    ///   (optional).
    ///
    /// * "parent": The expression ID that contains this expression (optional).
    ///
    /// A Python script for rendering this output as a GraphViz visualization is
    /// available at https://gist.github.com/kgutwin/efe5f03df5ff930d899249018a0a551b.
    Lineage {
        #[command(flatten)]
        io_args: IoArgs,
        #[arg(value_enum, long, default_value = "yaml")]
        format: Format,
    },

    /// Print info about the AST data structure
    Ast,

    /// Print JSON Schema
    JsonSchema {
        #[arg(value_enum, long)]
        schema_type: SchemaType,
    },
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

#[derive(Copy, Clone, Debug, Default)]
struct LoggingHelp;

impl LogLevel for LoggingHelp {
    /// By default, this will only report errors.
    fn default() -> Option<log::Level> {
        Some(log::Level::Error)
    }
    fn verbose_help() -> Option<&'static str> {
        Some("Increase logging verbosity")
    }

    fn verbose_long_help() -> Option<&'static str> {
        Some(
            r#"More `v`s, More vebose logging:
-v shows warnings
-vv shows info
-vvv shows debug
-vvvv shows trace"#,
        )
    }

    fn quiet_help() -> Option<&'static str> {
        Some("Silences logging output")
    }

    fn quiet_long_help() -> Option<&'static str> {
        Some("Silences logging output")
    }
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum Format {
    Json,
    Yaml,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum SchemaType {
    Pl,
    Rq,
    Lineage,
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
            Command::Debug(DebugCommand::JsonSchema { schema_type }) => {
                let schema = match schema_type {
                    SchemaType::Pl => schema_for!(pl::ModuleDef),
                    SchemaType::Rq => schema_for!(rq::RelationalQuery),
                    SchemaType::Lineage => schema_for!(FrameCollector),
                };
                io::stdout().write_all(
                    &serde_json::to_string_pretty(&schema)?.into_bytes()
                )?;
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
            Command::Lex { format, .. } => {
                let s = sources.sources.values().exactly_one().or_else(|_| {
                    // TODO: allow multiple sources
                    bail!("Currently `lex` only works with a single source, but found multiple sources")
                })?;
                let tokens = prql_to_tokens(s)?;
                match format {
                    Format::Json => serde_json::to_string_pretty(&tokens)?.into_bytes(),
                    Format::Yaml => serde_yaml::to_string(&tokens)?.into_bytes(),
                }
            }
            Command::Collect(_) => {
                let mut root_module_def = prql_to_pl_tree(sources)?;

                drop_module_def(&mut root_module_def.stmts, "std");

                pl_to_prql(&root_module_def)?.into_bytes()
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
                    semantic::reporting::collect_frames(*main.clone().into_relation_var().unwrap())
                        .frames
                } else {
                    vec![]
                };

                // combine with source
                combine_prql_and_frames(&source, frames).as_bytes().to_vec()
            }
            Command::Debug(DebugCommand::Lineage { format, .. }) => {
                let stmts = prql_to_pl_tree(sources)?;
                let fc = pl_to_lineage(stmts)?;

                match format {
                    Format::Json => serde_json::to_string_pretty(&fc)?.into_bytes(),
                    Format::Yaml => serde_yaml::to_string(&fc)?.into_bytes(),
                }
            }
            Command::Experimental(ExperimentalCommand::GenerateDocs(_)) => {
                let module_ref = prql_to_pl_tree(sources)?;

                docs_generator::generate_markdown_docs(module_ref.stmts).into_bytes()
            }
            Command::Compile {
                signature_comment,
                format,
                target,
                debug_log,
                ..
            } => {
                if debug_log.is_some() {
                    debug::log_start();
                }

                let opts = Options::default()
                    .with_target(Target::from_str(target).map_err(prqlc::ErrorMessages::from)?)
                    .with_signature_comment(*signature_comment)
                    .with_format(*format);

                let res = prql_to_pl_tree(sources)
                    .and_then(|pl| {
                        pl_to_rq_tree(pl, &main_path, &[semantic::NS_DEFAULT_DB.to_string()])
                    })
                    .and_then(|rq| rq_to_sql(rq, &opts))
                    .map_err(|e| e.composed(sources));

                if let Some(path) = debug_log {
                    write_log(path)?;
                }

                res?.as_bytes().to_vec()
            }
            _ => unreachable!("Other commands shouldn't reach `execute`"),
        })
    }

    fn read_input(&mut self) -> Result<(SourceTree, String)> {
        // Possibly this should be called by the relevant subcommands passing in
        // `input`, rather than matching on them and grabbing `input` from
        // `self`? But possibly if everything moves to `io_args`, then this is
        // quite reasonable?
        use Command::*;
        let io_args = match self {
            Parse { io_args, .. }
            | Lex { io_args, .. }
            | Collect(io_args)
            | Compile { io_args, .. }
            | Debug(DebugCommand::Annotate(io_args) | DebugCommand::Lineage { io_args, .. }) => {
                io_args
            }
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
        if input.path() == Path::new("-") && std::io::stdin().is_terminal() {
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
        use Command::{Collect, Compile, Debug, Experimental, Lex, Parse};
        let mut output = match self {
            Parse { io_args, .. }
            | Lex { io_args, .. }
            | Collect(io_args)
            | Compile { io_args, .. }
            | Debug(DebugCommand::Annotate(io_args) | DebugCommand::Lineage { io_args, .. }) => {
                io_args.output.clone()
            }
            Experimental(ExperimentalCommand::GenerateDocs(io_args)) => io_args.output.clone(),
            _ => unreachable!(),
        };
        output.write_all(data)
    }
}

pub fn write_log(path: &std::path::Path) -> Result<()> {
    let debug_log = if let Some(debug_log) = debug::log_finish() {
        debug_log
    } else {
        return Err(anyhow!(
            "debug log was started, but it cannot be found after compilation"
        ));
    };
    match path.extension().and_then(|s| s.to_str()) {
        Some("json") => {
            let file = BufWriter::new(File::create(path)?);
            serde_json::to_writer(file, &debug_log)?;
        }
        Some("html") => {
            let file = BufWriter::new(File::create(path)?);
            debug::render_log_to_html(file, &debug_log)?;
        }
        _ => {
            return Err(anyhow!("unknown debug log format for file {path:?}"));
        }
    }
    Ok(())
}

fn drop_module_def(stmts: &mut Vec<pr::Stmt>, name: &str) {
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

fn combine_prql_and_frames(source: &str, frames: Vec<(Option<pr::Span>, pl::Lineage)>) -> String {
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
        if let Some(span) = span {
            let line_len = source.get_line_range(&Range::from(span)).end - 1;

            while printed_lines_count < line_len {
                result.push(
                    source
                        .get_line_text(source.line(printed_lines_count).unwrap())
                        .unwrap()
                        // Ariadne 0.4.1 added a line break at the end of the line, so we
                        // trim it.
                        .trim_end()
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
                // Ariadne 0.4.1 added a line break at the end of the line, so we
                // trim it.
                .trim_end()
                .to_string();
            printed_lines_count += 1;

            result.push(format!("{chars:width$} # {frame}"));
        }
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
from initial_table
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
        from initial_table
        select {f = first_name, l = last_name, gender}  # [f, l, initial_table.gender]
        derive full_name = f"{f} {l}"                   # [f, l, initial_table.gender, full_name]
        take 23                                         # [f, l, initial_table.gender, full_name]
        select {f"{l} {f}", full = full_name, gender}   # [?, full, initial_table.gender]
        sort full                                       # [?, full, initial_table.gender]
        "###);
    }

    /// Check we get an error on a bad input
    #[test]
    fn compile_bad() {
        anstream::ColorChoice::Never.write_global();

        let result = Command::execute(
            &Command::Compile {
                io_args: IoArgs::default(),
                signature_comment: false,
                format: true,
                target: "sql.any".to_string(),
                debug_log: None,
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
    fn compile() {
        let result = Command::execute(
            &Command::Compile {
                io_args: IoArgs::default(),
                signature_comment: false,
                format: true,
                target: "sql.any".to_string(),
                debug_log: None,
            },
            &mut SourceTree::new(
                [
                    ("Project.prql".into(), "orders.x | select y".to_string()),
                    (
                        "orders.prql".into(),
                        "let x = (from z | select {y, u})".to_string(),
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
                      Ident: from
                      span: 1:0-4
                    args:
                    - Ident: x
                      span: 1:5-6
                  span: 1:0-6
                - FuncCall:
                    name:
                      Ident: select
                      span: 1:9-15
                    args:
                    - Ident: y
                      span: 1:16-17
                  span: 1:9-17
              span: 1:0-17
          span: 1:0-17
        "###);
    }
    #[test]
    fn lex() {
        let output = Command::execute(
            &Command::Lex {
                io_args: IoArgs::default(),
                format: Format::Yaml,
            },
            &mut "from x | select y".into(),
            "",
        )
        .unwrap();

        // TODO: terser output; maybe serialize span as `0..4`? Remove the
        // `!Ident` complication?
        assert_snapshot!(String::from_utf8(output).unwrap().trim(), @r###"
        - kind: !Ident from
          span:
            start: 0
            end: 4
        - kind: !Ident x
          span:
            start: 5
            end: 6
        - kind: !Control '|'
          span:
            start: 7
            end: 8
        - kind: !Ident select
          span:
            start: 9
            end: 15
        - kind: !Ident y
          span:
            start: 16
            end: 17
        "###);
    }
    #[test]
    fn lex_nested_enum() {
        let output = Command::execute(
            &Command::Lex {
                io_args: IoArgs::default(),
                format: Format::Yaml,
            },
            &mut r#"
            from tracks
            take 10
            "#
            .into(),
            "",
        )
        .unwrap();

        assert_snapshot!(String::from_utf8(output).unwrap().trim(), @r###"
        - kind: NewLine
          span:
            start: 0
            end: 1
        - kind: !Ident from
          span:
            start: 13
            end: 17
        - kind: !Ident tracks
          span:
            start: 18
            end: 24
        - kind: NewLine
          span:
            start: 24
            end: 25
        - kind: !Ident take
          span:
            start: 37
            end: 41
        - kind: !Literal
            Integer: 10
          span:
            start: 42
            end: 44
        - kind: NewLine
          span:
            start: 44
            end: 45
        "###);
    }
}
