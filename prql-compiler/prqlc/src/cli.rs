use anyhow::Result;
use ariadne::Source;
use clap::{Parser, Subcommand};
use clio::{Input, Output};
use itertools::Itertools;
use std::io::{Read, Write};
use std::ops::Range;
use std::process::exit;
use std::str::FromStr;

use prql_compiler::semantic::{self, reporting::*};
use prql_compiler::{ast::pl::Frame, pl_to_prql};
use prql_compiler::{compile, prql_to_pl, Span};
use prql_compiler::{downcast, Options, Target};

use crate::watch;

/// Entrypoint called by [crate::main]
pub fn main() -> color_eyre::eyre::Result<()> {
    env_logger::builder().format_timestamp(None).init();
    color_eyre::install()?;
    let mut cli = Cli::parse();
    cli.color.apply();

    if let Err(error) = cli.command.run() {
        eprintln!("{error}");
        exit(1)
    }

    Ok(())
}

#[derive(Parser, Debug, Clone)]
#[command(color = concolor_clap::color_choice())]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[command(flatten)]
    color: concolor_clap::Color,
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
    Format(IoArgs),

    /// Parse, resolve & combine source with comments annotating relation type
    Annotate(IoArgs),

    /// Parse & resolve, but don't lower into RQ
    Debug(IoArgs),

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
        #[arg(long, default_value = "true")]
        include_signature_comment: bool,
        #[arg(short, long, default_value = "sql.any", env = "PRQLC_TARGET")]
        target: String,
    },

    /// Watch a directory and compile .prql files to .sql files
    Watch(watch::WatchArgs),

    /// Show available compile target names
    #[command(name = "list-targets")]
    ListTargets,
}

#[derive(clap::Args, Default, Debug, Clone)]
pub struct IoArgs {
    #[arg(value_parser, default_value = "-")]
    input: Input,

    #[arg(value_parser, default_value = "-")]
    output: Output,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum Format {
    Json,
    Yaml,
}

fn is_stdin(input: &Input) -> bool {
    input.path() == "-"
}

impl Command {
    /// Entrypoint called by [`main`]
    pub fn run(&mut self) -> Result<()> {
        match self {
            Command::Watch(command) => watch::run(command),
            Command::ListTargets => self.list_targets(),
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
        let (source, source_id) = self.read_input()?;

        let res = self.execute(&source);

        match res {
            Ok(buf) => {
                self.write_output(&buf)?;
            }
            Err(e) => {
                print!(
                    "{:}",
                    // TODO: we're repeating this for `Compile`; can we consolidate?
                    downcast(e).composed(
                        &source_id,
                        &source,
                        concolor::get(concolor::Stream::Stdout).ansi_color()
                    )
                );
                std::process::exit(1)
            }
        }

        Ok(())
    }

    fn execute(&self, source: &str) -> Result<Vec<u8>> {
        Ok(match self {
            Command::Parse { format, .. } => {
                let ast = prql_to_pl(source)?;
                match format {
                    Format::Json => serde_json::to_string_pretty(&ast)?.into_bytes(),
                    Format::Yaml => serde_yaml::to_string(&ast)?.into_bytes(),
                }
            }
            Command::Format(_) => prql_to_pl(source).and_then(pl_to_prql)?.as_bytes().to_vec(),
            Command::Debug(_) => {
                let stmts = prql_to_pl(source)?;
                let context = semantic::resolve_only(stmts, None)?;

                let references = label_references(&context, "".to_string(), source.to_string());

                [references, format!("\n{context:#?}\n").into_bytes()].concat()
            }
            Command::Annotate(_) => {
                // TODO: potentially if there is code performing a role beyond
                // presentation, it should be a library function; and we could
                // promote it to the `prql-compiler` crate.
                let stmts = prql_to_pl(source)?;

                // resolve
                let ctx = semantic::resolve_only(stmts, None)?;

                let frames = if let Some((main, _)) = ctx.find_main(&[]) {
                    collect_frames(main.clone())
                } else {
                    vec![]
                };

                // combine with source
                combine_prql_and_frames(source, frames).as_bytes().to_vec()
            }
            Command::Resolve { format, .. } => {
                let ast = prql_to_pl(source)?;
                let ir = semantic::resolve(ast)?;
                match format {
                    Format::Json => serde_json::to_string_pretty(&ir)?.into_bytes(),
                    Format::Yaml => serde_yaml::to_string(&ir)?.into_bytes(),
                }
            }
            Command::SQLCompile {
                include_signature_comment,
                target,
                ..
            } => compile(
                source,
                &Options::default()
                    .with_target(Target::from_str(target).map_err(|e| downcast(e.into()))?)
                    .with_color(concolor::get(concolor::Stream::Stdout).ansi_color())
                    .with_signature_comment(*include_signature_comment),
            )?
            .as_bytes()
            .to_vec(),

            Command::SQLPreprocess { .. } => {
                let ast = prql_to_pl(source)?;
                let rq = semantic::resolve(ast)?;
                let srq = prql_compiler::sql::internal::preprocess(rq)?;
                format!("{srq:#?}").as_bytes().to_vec()
            }
            Command::SQLAnchor { format, .. } => {
                let ast = prql_to_pl(source)?;
                let rq = semantic::resolve(ast)?;
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

    fn read_input(&mut self) -> Result<(String, String)> {
        // Possibly this should be called by the relevant subcommands passing in
        // `input`, rather than matching on them and grabbing `input` from
        // `self`? But possibly if everything moves to `io_args`, then this is
        // quite reasonable?
        use Command::*;
        let mut input = match self {
            Parse { io_args, .. }
            | Resolve { io_args, .. }
            | SQLCompile { io_args, .. }
            | SQLPreprocess(io_args)
            | SQLAnchor { io_args, .. } => io_args.input.clone(),
            Format(io) | Debug(io) | Annotate(io) => io.input.clone(),
            _ => unreachable!(),
        };
        // Don't wait without a prompt when running `prqlc compile` —
        // it's confusing whether it's waiting for input or not. This
        // offers the prompt.
        if is_stdin(&input) && atty::is(atty::Stream::Stdin) {
            println!("Enter PRQL, then ctrl-d:\n");
        }

        let mut source = String::new();
        input.read_to_string(&mut source)?;
        let source_id = (input.path()).to_str().unwrap().to_string();
        Ok((source, source_id))
    }

    fn write_output(&mut self, data: &[u8]) -> std::io::Result<()> {
        use Command::*;
        let mut output = match self {
            Parse { io_args, .. }
            | Resolve { io_args, .. }
            | SQLCompile { io_args, .. }
            | SQLAnchor { io_args, .. }
            | SQLPreprocess(io_args) => io_args.output.to_owned(),
            Format(io) | Debug(io) | Annotate(io) => io.output.to_owned(),
            _ => unreachable!(),
        };
        output.write_all(data)
    }
}

fn combine_prql_and_frames(source: &str, frames: Vec<(Span, Frame)>) -> String {
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

#[cfg(test)]
mod tests {
    use insta::{assert_display_snapshot, assert_snapshot};

    // TODO: would be good to test the basic CLI interface — i.e. snapshotting this:

    // $ prqlc parse --help
    //
    // Parse PL AST
    //
    // Usage: prqlc parse [OPTIONS] [INPUT] [OUTPUT]
    //
    // Arguments:
    //   [INPUT]   [default: -]
    //   [OUTPUT]  [default: -]
    //
    // Options:
    //       --format <FORMAT>  [possible values: json, yaml]
    //   -h, --help             Print help

    use super::*;

    #[test]
    fn layouts() {
        let output = Command::execute(
            &Command::Annotate(IoArgs::default()),
            r#"
from initial_table
select [f = first_name, l = last_name, gender]
derive full_name = f + " " + l
take 23
select [l + " " + f, full = full_name, gender]
sort full
        "#,
        )
        .unwrap();
        assert_snapshot!(String::from_utf8(output).unwrap().trim(),
        @r###"
        from initial_table
        select [f = first_name, l = last_name, gender]  # [f, l, initial_table.gender]
        derive full_name = f + " " + l                  # [f, l, initial_table.gender, full_name]
        take 23                                         # [f, l, initial_table.gender, full_name]
        select [l + " " + f, full = full_name, gender]  # [?, full, initial_table.gender]
        sort full                                       # [?, full, initial_table.gender]
        "###);
    }

    #[test]
    fn format() {
        let output = Command::execute(
            &Command::Format(IoArgs::default()),
            r#"
from table.subdivision
 derive      `želva_means_turtle`   =    (`column with spaces` + 1) * 3
group a_column (take 10 | sort b_column | derive [the_number = rank, last = lag 1 c_column] )
        "#,
        )
        .unwrap();

        // this test is here just to document behavior - the result is far from being correct:
        // - indentation does not stack
        // - operator precedence is not considered (parenthesis are not inserted for numerical
        //   operations but are always inserted for function calls)
        assert_snapshot!(String::from_utf8(output).unwrap().trim(),
        @r###"
        from table.subdivision
        derive `želva_means_turtle` = `column with spaces` + 1 * 3
        group a_column (
          take 10
          sort b_column
          derive [
          the_number = rank,
          last = lag 1 c_column,
        ]
        )
        "###);
    }

    #[test]
    fn compile() {
        // Check we get an error on a bad input
        let input = "asdf";

        // Disable colors (would be better if this were a proper CLI test and
        // passed in `--color=never`)
        concolor_clap::Color {
            color: concolor_clap::ColorChoice::Never,
        }
        .apply();

        let result = Command::execute(
            &Command::SQLCompile {
                io_args: IoArgs::default(),
                include_signature_comment: true,
                target: "sql.any".to_string(),
            },
            input,
        );
        assert_display_snapshot!(result.unwrap_err(), @r###"
        Error:
           ╭─[:1:1]
           │
         1 │ asdf
           │ ──┬─
           │   ╰─── Unknown name asdf
        ───╯
        "###);
    }

    #[test]
    fn parse() {
        let output = Command::execute(
            &Command::Parse {
                io_args: IoArgs::default(),
                format: Format::Yaml,
            },
            "from x | select y",
        )
        .unwrap();

        assert_display_snapshot!(String::from_utf8(output).unwrap().trim(), @r###"
        - name: main
          Main:
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
        "###);
    }
    #[test]
    fn resolve() {
        let output = Command::execute(
            &Command::Resolve {
                io_args: IoArgs::default(),
                format: Format::Yaml,
            },
            "from x | select y",
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
            "from employees | sort salary | take 3 | filter salary > 0",
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
                  alias: employees
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
              alias: table_0
              riid: 1
          - Select:
            - 2
            - 3
          - Filter:
              kind:
                BuiltInFunction:
                  name: std.gt
                  args:
                  - kind:
                      ColumnRef: 2
                    span: span-chars-47-53
                  - kind:
                      Literal:
                        Integer: 0
                    span: span-chars-56-57
              span: span-chars-47-57
          - Sort:
            - direction: Asc
              column: 2
        "###);
    }
}
