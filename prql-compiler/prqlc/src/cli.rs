use anstream::eprintln;
use anyhow::bail;
use anyhow::Result;
use ariadne::Source;
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clio::Output;
use itertools::Itertools;
use prql_compiler::ast::pl::StmtKind;
use std::io::Write;
use std::ops::Range;
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;
use std::{env, fs::File};

use prql_compiler::semantic::{self, reporting::*};
use prql_compiler::{ast::pl::Lineage, pl_to_prql};
use prql_compiler::{downcast, Options, Target};
use prql_compiler::{pl_to_rq_tree, prql_to_pl, prql_to_pl_tree, rq_to_sql, SourceTree, Span};

use crate::watch;

/// Entrypoint called by [crate::main]
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
        input: clio_extended::Input,
    },

    #[command(subcommand)]
    Debug(DebugCommand),

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
        #[arg(long, action = clap::ArgAction::SetFalse)]
        hide_signature_comment: bool,
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
    /// Parse & resolve, but don't lower into RQ
    Semantics(IoArgs),

    /// Parse & evaluate expression down to a value
    ///
    /// Cannot contain references to tables or any other outside sources.
    /// Meant as a playground for testing out language design decisions.
    Eval(IoArgs),

    /// Parse, resolve & combine source with comments annotating relation type
    Annotate(IoArgs),
}

#[derive(clap::Args, Default, Debug, Clone)]
pub struct IoArgs {
    #[arg(value_parser, default_value = "-", value_hint(ValueHint::AnyPath))]
    input: clio_extended::Input,

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
                let text = input.read_to_tree()?;
                let (_, source) = text.sources.clone().into_iter().exactly_one().or_else(
                    |_| bail!(
                        "Currently `fmt` only works with a single source, but found multiple sources: {:?}",
                        text.sources.keys()
                            .map(|x| x.display().to_string())
                            .sorted()
                            .map(|x| format!("`{}`", x))
                            .join(", ")
                    )
                )?;
                let ast = prql_to_pl(&source)?;
                let mut output: Output = match input {
                    clio_extended::Input::Stdin(_) => Output::Stdout(std::io::stdout()),
                    clio_extended::Input::Pipe(_, _) => Output::Stdout(std::io::stdout()),
                    // Pass a path and a file pointing to that path
                    clio_extended::Input::File(path, _) => Output::File(
                        path.as_os_str().into(),
                        File::options().write(true).open(input.path())?,
                    ),
                    clio_extended::Input::Directory(_) => {
                        bail!("Cannot format a directory yet, please use `prqlc fmt <file>`")
                    }
                };

                output.write_all(&pl_to_prql(ast)?.into_bytes())?;
                Ok(())
            }
            Command::ShellCompletion { shell } => {
                shell.generate(&mut Cli::command(), &mut std::io::stdout());
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
            Command::Debug(DebugCommand::Semantics(_)) => {
                semantic::load_std_lib(sources);
                let stmts = prql_to_pl_tree(sources)?;

                let context = semantic::resolve(stmts, Default::default())
                    .map_err(prql_compiler::downcast)
                    .map_err(|e| e.composed(sources))?;

                let mut out = Vec::new();
                for (source_id, source) in &sources.sources {
                    let source_id = source_id.to_str().unwrap().to_string();
                    out.extend(label_references(&context, source_id, source.clone()));
                }

                out.extend(format!("\n{context:#?}\n").into_bytes());
                out
            }
            Command::Debug(DebugCommand::Annotate(_)) => {
                let (_, source) = sources.sources.clone().into_iter().exactly_one().or_else(
                    |_| bail!(
                        "Currently `annotate` only works with a single source, but found multiple sources: {:?}",
                        sources.sources.keys()
                            .map(|x| x.display().to_string())
                            .sorted()
                            .map(|x| format!("`{}`", x))
                            .join(", ")
                    )
                )?;

                // TODO: potentially if there is code performing a role beyond
                // presentation, it should be a library function; and we could
                // promote it to the `prql-compiler` crate.
                let stmts = prql_to_pl(&source)?;

                // resolve
                let stmts = SourceTree::single(PathBuf::new(), stmts);
                let ctx = semantic::resolve(stmts, Default::default())?;

                let frames = if let Ok((main, _)) = ctx.find_main_rel(&[]) {
                    collect_frames(*main.clone().into_relation_var().unwrap())
                } else {
                    vec![]
                };

                // combine with source
                combine_prql_and_frames(&source, frames).as_bytes().to_vec()
            }
            Command::Debug(DebugCommand::Eval(_)) => {
                let stmts = prql_to_pl_tree(sources)?;

                let mut res = String::new();

                for (path, stmts) in stmts.sources {
                    res += &format!("# {}\n\n", path.to_str().unwrap());

                    for stmt in stmts {
                        if let StmtKind::VarDef(def) = stmt.kind {
                            res += &format!("## {}\n", stmt.name);

                            let val = semantic::eval(*def.value)
                                .map_err(downcast)
                                .map_err(|e| e.composed(sources))?;
                            res += &val.to_string();
                            res += "\n\n";
                        }
                    }
                }

                res.into_bytes()
            }
            Command::Resolve { format, .. } => {
                semantic::load_std_lib(sources);

                let ast = prql_to_pl_tree(sources)?;
                let ir = pl_to_rq_tree(ast, &main_path)?;

                match format {
                    Format::Json => serde_json::to_string_pretty(&ir)?.into_bytes(),
                    Format::Yaml => serde_yaml::to_string(&ir)?.into_bytes(),
                }
            }
            Command::SQLCompile {
                hide_signature_comment,
                target,
                ..
            } => {
                semantic::load_std_lib(sources);

                let opts = Options::default()
                    .with_target(Target::from_str(target).map_err(|e| downcast(e.into()))?)
                    .with_signature_comment(*hide_signature_comment);

                prql_to_pl_tree(sources)
                    .and_then(|pl| pl_to_rq_tree(pl, &main_path))
                    .and_then(|rq| rq_to_sql(rq, &opts))
                    .map_err(|e| e.composed(sources))?
                    .as_bytes()
                    .to_vec()
            }

            Command::SQLPreprocess { .. } => {
                semantic::load_std_lib(sources);

                let ast = prql_to_pl_tree(sources)?;
                let rq = pl_to_rq_tree(ast, &main_path)?;
                let srq = prql_compiler::sql::internal::preprocess(rq)?;
                format!("{srq:#?}").as_bytes().to_vec()
            }
            Command::SQLAnchor { format, .. } => {
                semantic::load_std_lib(sources);

                let ast = prql_to_pl_tree(sources)?;
                let rq = pl_to_rq_tree(ast, &main_path)?;
                let srq = prql_compiler::sql::internal::anchor(rq)?;

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
        use Command::*;
        let io_args = match self {
            Parse { io_args, .. }
            | Resolve { io_args, .. }
            | SQLCompile { io_args, .. }
            | SQLPreprocess(io_args)
            | SQLAnchor { io_args, .. }
            | Debug(DebugCommand::Semantics(io_args))
            | Debug(DebugCommand::Annotate(io_args))
            | Debug(DebugCommand::Eval(io_args)) => io_args,
            _ => unreachable!(),
        };
        let input = &mut io_args.input;

        // Don't wait without a prompt when running `prqlc compile` —
        // it's confusing whether it's waiting for input or not. This
        // offers the prompt.
        if input.is_stdin() && atty::is(atty::Stream::Stdin) {
            #[cfg(unix)]
            eprintln!("Enter PRQL, then press ctrl-d to compile:\n");
            #[cfg(windows)]
            eprintln!("Enter PRQL, then press ctrl-z to compile:\n");
        }

        let file_tree = input.read_to_tree()?;

        let main_path = io_args.main_path.clone().unwrap_or_default();

        Ok((file_tree, main_path))
    }

    fn write_output(&mut self, data: &[u8]) -> std::io::Result<()> {
        use Command::*;
        let mut output = match self {
            Parse { io_args, .. }
            | Resolve { io_args, .. }
            | SQLCompile { io_args, .. }
            | SQLAnchor { io_args, .. }
            | SQLPreprocess(io_args)
            | Debug(DebugCommand::Semantics(io_args))
            | Debug(DebugCommand::Annotate(io_args))
            | Debug(DebugCommand::Eval(io_args)) => io_args.output.to_owned(),
            _ => unreachable!(),
        };
        output.write_all(data)
    }
}

fn combine_prql_and_frames(source: &str, frames: Vec<(Span, Lineage)>) -> String {
    let source = Source::from(source);
    let lines = source.lines().collect_vec();
    let width = lines.iter().map(|l| l.len()).max().unwrap_or(0);

    let mut printed_lines = 0;
    let mut result = Vec::new();
    for (span, frame) in frames {
        let line = source.get_line_range(&Range::from(span)).end - 1;

        while printed_lines < line {
            result.push(lines[printed_lines].chars().collect());
            printed_lines += 1;
        }

        if printed_lines >= lines.len() {
            break;
        }
        let chars: String = lines[printed_lines].chars().collect();
        printed_lines += 1;

        result.push(format!("{chars:width$} # {frame}"));
    }
    for line in lines.iter().skip(printed_lines) {
        result.push(line.chars().collect());
    }

    result.into_iter().join("\n") + "\n"
}

/// [clio::Input], extended to also allow consuming directories
mod clio_extended {
    use std::collections::HashMap;
    use std::ffi::OsStr;
    use std::fs::{self, File};
    use std::io::{self, Read, Stdin};
    use std::marker::PhantomData;
    use std::path::PathBuf;

    use clap::builder::TypedValueParser;
    use prql_compiler::SourceTree;
    use walkdir::WalkDir;

    #[derive(Debug)]
    pub enum Input {
        /// a [`Stdin`] when the path was `-`
        Stdin(Stdin),
        /// a [`File`] representing the named pipe e.g. if called with `<(cat /dev/null)`
        Pipe(PathBuf, File),
        /// a normal [`File`] opened from the path
        File(PathBuf, File),
        /// a Directory
        Directory(PathBuf),
    }

    impl Input {
        /// Constructs a new input either by opening the file or for '-' returning stdin
        pub fn new<S: AsRef<OsStr>>(path: S) -> clio::Result<Self> {
            let path = path.as_ref();
            if path == "-" {
                Ok(Self::std())
            } else {
                let pathbuf = PathBuf::from(path);
                if pathbuf.is_dir() {
                    return Ok(Input::Directory(pathbuf));
                }
                let file = File::open(&pathbuf)?;
                if is_fifo(&file)? {
                    Ok(Input::Pipe(pathbuf, file))
                } else {
                    Ok(Input::File(pathbuf, file))
                }
            }
        }

        /// Constructs a new input for stdin
        pub fn std() -> Self {
            Input::Stdin(io::stdin())
        }

        pub fn is_stdin(&self) -> bool {
            matches!(self, Input::Stdin(_))
        }

        /// Returns the path/url used to create the input
        pub fn path(&self) -> &OsStr {
            match self {
                Input::Stdin(_) => "-".as_ref(),
                Input::Pipe(pathbuf, _) | Input::File(pathbuf, _) | Input::Directory(pathbuf) => {
                    pathbuf.as_os_str()
                }
            }
        }

        pub fn read_to_tree(&mut self) -> anyhow::Result<SourceTree<String>> {
            let mut only_file = String::new();

            match self {
                Input::Stdin(stdin) => stdin.read_to_string(&mut only_file)?,
                Input::Pipe(_, pipe) => pipe.read_to_string(&mut only_file)?,
                Input::File(_, file) => file.read_to_string(&mut only_file)?,
                Input::Directory(root_path) => {
                    // special case: actually walk the dirs
                    let mut sources = HashMap::new();
                    for entry in WalkDir::new(&root_path) {
                        let entry = entry.unwrap();
                        let path = entry.path();

                        if path.is_file() && path.extension() == Some(OsStr::new("prql")) {
                            let file_contents = fs::read_to_string(path)?;
                            let path = path.strip_prefix(&root_path)?.to_path_buf();

                            sources.insert(path, file_contents);
                        }
                    }

                    return Ok(SourceTree::new(sources));
                }
            };

            let path = PathBuf::from("Root.prql");
            Ok(SourceTree::single(path, only_file))
        }
    }

    impl Default for Input {
        fn default() -> Self {
            Input::std()
        }
    }

    /// Opens a new handle on the file from the path that was used to create it
    /// Probably a very bad idea to have two handles to the same file
    ///
    /// This will panic if the file has been deleted
    ///
    /// Only included when using the `clap-parse` feature as it is needed for `value_parser`
    impl Clone for Input {
        fn clone(&self) -> Self {
            Input::new(self.path()).unwrap()
        }
    }

    impl clap::builder::ValueParserFactory for Input {
        type Parser = OsStrParser<Input>;
        fn value_parser() -> Self::Parser {
            OsStrParser::new()
        }
    }

    /// A clap parser that converts [`&OsStr`](std::ffi::OsStr) to an [Input].
    #[derive(Copy, Clone, Debug)]
    pub struct OsStrParser<T> {
        phantom: PhantomData<T>,
    }

    impl<T> OsStrParser<T> {
        pub(crate) fn new() -> Self {
            OsStrParser {
                phantom: PhantomData,
            }
        }
    }

    impl TypedValueParser for OsStrParser<Input> {
        type Value = Input;

        fn parse_ref(
            &self,
            cmd: &clap::Command,
            arg: Option<&clap::Arg>,
            value: &std::ffi::OsStr,
        ) -> core::result::Result<Self::Value, clap::Error> {
            Input::new(value).map_err(|orig| {
                cmd.clone().error(
                    clap::error::ErrorKind::InvalidValue,
                    if let Some(arg) = arg {
                        format!(
                            "Invalid value for {}: Could not open {:?}: {}",
                            arg, value, orig
                        )
                    } else {
                        format!("Could not open {:?}: {}", value, orig)
                    },
                )
            })
        }
    }

    #[cfg(not(unix))]
    fn is_fifo(_: &File) -> clio::Result<bool> {
        Ok(false)
    }

    #[cfg(unix)]
    fn is_fifo(file: &File) -> clio::Result<bool> {
        use std::os::unix::fs::FileTypeExt;
        Ok(file.metadata()?.file_type().is_fifo())
    }
}

/// Unit tests for `prqlc`. Integration tests (where we call the actual binary)
/// are in `prql-compiler/prqlc/tests/test.rs`.
#[cfg(test)]
mod tests {
    use insta::{assert_display_snapshot, assert_snapshot};

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

    #[ignore = "Need to write a fmt test with the full CLI when insta_cmd is fixed"]
    #[test]
    fn format() {
        // This is the previous previous approach with the Format command; which
        // now doesn't run through `execute`; instead through `run`.
        let output = Command::execute(
            &Command::Format {
                input: clio_extended::Input::default(),
            },
            &mut r#"
from table.subdivision
 derive      `želva_means_turtle`   =    (`column with spaces` + 1) * 3
group a_column (take 10 | sort b_column | derive {the_number = rank, last = lag 1 c_column} )
        "#
            .into(),
            "",
        )
        .unwrap();

        // this test is here just to document behavior - the result is far from being correct:
        // - indentation does not stack
        // - operator precedence is not considered (parenthesis are not inserted for numerical
        //   operations but are always inserted for function calls)
        assert_snapshot!(String::from_utf8(output).unwrap().trim(),
        @r###"
        from table.subdivision
        derive `želva_means_turtle` = (`column with spaces` + 1) * 3
        group a_column (
          take 10
          sort b_column
          derive {the_number = rank, last = lag 1 c_column}
        )
        "###);
    }

    /// Check we get an error on a bad input
    #[test]
    fn compile() {
        // Disable colors (would be better if this were a proper CLI test and
        // passed in `--color=never`)
        anstream::ColorChoice::Never.write_global();

        let result = Command::execute(
            &Command::SQLCompile {
                io_args: IoArgs::default(),
                hide_signature_comment: true,
                target: "sql.any".to_string(),
            },
            &mut "asdf".into(),
            "",
        );

        assert_display_snapshot!(&result.unwrap_err().to_string(), @r###"
        Error:
           ╭─[:1:1]
           │
         1 │ asdf
           │ ──┬─
           │   ╰─── Unknown name
        ───╯
        "###);
    }

    #[test]
    fn compile_multiple() {
        let result = Command::execute(
            &Command::SQLCompile {
                io_args: IoArgs::default(),
                hide_signature_comment: true,
                target: "sql.any".to_string(),
            },
            &mut SourceTree::new([
                ("Project.prql".into(), "orders.x | select y".to_string()),
                (
                    "orders.prql".into(),
                    "let x = (from z | select {y, u})".to_string(),
                ),
            ]),
            "main",
        )
        .unwrap();
        assert_display_snapshot!(String::from_utf8(result).unwrap().trim(), @r###"
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

        -- Generated by PRQL compiler version:0.8.1 (https://prql-lang.org)
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

        assert_display_snapshot!(String::from_utf8(output).unwrap().trim(), @r###"
        sources:
          '':
          - name: main
            VarDef:
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
              ty_expr: null
              kind: Main
            annotations: []
        source_ids:
          1: ''
        "###);
    }
    #[test]
    fn resolve() {
        let output = Command::execute(
            &Command::Resolve {
                io_args: IoArgs::default(),
                format: Format::Yaml,
            },
            &mut "from x | select y".into(),
            "",
        )
        .unwrap();

        assert_display_snapshot!(String::from_utf8(output).unwrap().trim(), @r###"
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
            &mut "from employees | sort salary | take 3 | filter salary > 0".into(),
            "",
        )
        .unwrap();

        assert_display_snapshot!(String::from_utf8(output).unwrap().trim(), @r###"
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
                    span: 1:47-53
                  - kind:
                      Literal:
                        Integer: 0
                    span: 1:56-57
              span: 1:47-57
          - Sort:
            - direction: Asc
              column: 2
        "###);
    }
}
