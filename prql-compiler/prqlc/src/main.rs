use anyhow::{anyhow, Result};
use ariadne::Source;
use clap::Parser;
use clio::{Input, Output};
use itertools::Itertools;
use std::io::{Read, Write};
use std::ops::Range;
use std::process::exit;

use prql_compiler::semantic::{self, reporting::*};

use prql_compiler::downcast;
use prql_compiler::{ast::pl::Frame, pl_to_prql};
use prql_compiler::{compile, prql_to_pl, Span};

fn main() -> color_eyre::eyre::Result<()> {
    env_logger::builder().format_timestamp(None).init();
    color_eyre::install()?;
    let mut cli = Cli::parse();

    if let Err(error) = cli.run() {
        eprintln!("{error}");
        exit(1)
    }

    Ok(())
}

#[derive(Parser)]
#[clap(name = env!("CARGO_PKG_NAME"), about, version)]
pub enum Cli {
    /// Parse PL AST
    Parse(CommandIO),

    /// Parse & generate PRQL code back
    #[clap(name = "fmt")]
    Format(CommandIO),

    /// Parse, resolve & combine source with comments annotating relation type
    Annotate(CommandIO),

    /// Parse & resolve, but don't lower into RQ
    Debug(CommandIO),

    /// Parse, resolve & lower into RQ
    Resolve(CommandIO),

    /// Parse, resolve, lower into RQ & compile to SQL
    Compile(CommandIO),
}

#[derive(clap::Args, Default)]
pub struct CommandIO {
    #[clap(value_parser, default_value = "-")]
    input: Input,

    #[clap(value_parser, default_value = "-")]
    output: Output,
}

fn is_stdin(input: &Input) -> bool {
    input.path() == "-"
}

impl Cli {
    pub fn run(&mut self) -> Result<()> {
        let (source, source_id) = self.read_input()?;

        let res = self.execute(&source);

        match res {
            Ok(buf) => {
                self.write_output(&buf)?;
            }
            Err(e) => {
                print!("{:}", downcast(e).composed(&source_id, &source, true));
                std::process::exit(1)
            }
        }

        Ok(())
    }

    fn execute(&self, source: &str) -> Result<Vec<u8>> {
        Ok(match self {
            Cli::Parse(_) => {
                let ast = prql_to_pl(source).map_err(|e| anyhow!(e))?;

                serde_yaml::to_string(&ast)?.into_bytes()
            }
            Cli::Format(_) => prql_to_pl(source)
                .and_then(pl_to_prql)
                .map_err(|x| anyhow!(x))?
                .as_bytes()
                .to_vec(),
            Cli::Debug(_) => {
                let stmts = prql_to_pl(source).map_err(|x| anyhow!(x))?;
                let (stmts, context) = semantic::resolve_only(stmts, None)?;

                let (references, stmts) =
                    label_references(stmts, &context, "".to_string(), source.to_string());

                [
                    references,
                    format!("\n{context:#?}\n").into_bytes(),
                    format!("\n{stmts:#?}\n").into_bytes(),
                ]
                .concat()
            }
            Cli::Annotate(_) => {
                let stmts = prql_to_pl(source).map_err(|x| anyhow!(x))?;

                // resolve
                let (stmts, _) = semantic::resolve_only(stmts, None)?;

                let frames = collect_frames(stmts);

                // combine with source
                combine_prql_and_frames(source, frames).as_bytes().to_vec()
            }
            Cli::Resolve(_) => {
                let ast = prql_to_pl(source).map_err(|x| anyhow!(x))?;
                let ir = semantic::resolve(ast)?;

                serde_json::to_string_pretty(&ir)?.into_bytes()
            }
            Cli::Compile(_) => compile(source, None)
                .map_or_else(|x| x.to_string(), |x| x)
                .as_bytes()
                .to_vec(),
        })
    }

    fn read_input(&mut self) -> Result<(String, String)> {
        use Cli::*;
        match self {
            Parse(io) | Format(io) | Debug(io) | Annotate(io) | Resolve(io) | Compile(io) => {
                // Don't wait without a prompt when running `prql-compiler compile` —
                // it's confusing whether it's waiting for input or not. This
                // offers the prompt.
                if is_stdin(&io.input) && atty::is(atty::Stream::Stdin) {
                    println!("Enter PRQL, then ctrl-d:");
                    println!();
                }

                let mut source = String::new();
                io.input.read_to_string(&mut source)?;
                let source_id = (*io.input.path()).to_str().unwrap().to_string();
                Ok((source, source_id))
            }
        }
    }

    fn write_output(&mut self, data: &[u8]) -> std::io::Result<()> {
        use Cli::*;
        match self {
            Parse(io) | Format(io) | Debug(io) | Annotate(io) | Resolve(io) | Compile(io) => {
                io.output.write_all(data)
            }
        }
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
    use insta::assert_snapshot;

    use super::*;

    #[test]
    fn layouts() {
        let output = Cli::execute(
            &Cli::Annotate(CommandIO::default()),
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
        select [f = first_name, l = last_name, gender]  # [f, l, gender]
        derive full_name = f + " " + l                  # [f, l, gender, full_name]
        take 23                                         # [f, l, gender, full_name]
        select [l + " " + f, full = full_name, gender]  # [?, full, gender]
        sort full                                       # [?, full, gender]
        "###);
    }

    #[test]
    fn format() {
        let output = Cli::execute(
            &Cli::Format(CommandIO::default()),
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
}
