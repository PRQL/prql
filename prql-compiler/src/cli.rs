use anyhow::Result;
use ariadne::Source;
use clap::Parser;
use clio::{Input, Output};
use itertools::Itertools;
use std::{
    io::{Read, Write},
    ops::Range,
};

use crate::ast::Frame;
use crate::error::{self, Span};
use crate::parse;
use crate::semantic::{self, Context};

#[derive(Parser)]
#[clap(name = env!("CARGO_PKG_NAME"), about, version)]
pub enum Cli {
    /// Produces abstract syntax tree
    Parse(CommandIO),

    /// Formats PRQL code
    #[clap(name = "fmt")]
    Format(CommandIO),

    /// Adds comments annotating current table layout
    Annotate(CommandIO),

    Debug(CommandIO),

    /// Produces intermediate representation of the query
    Resolve(CommandIO),

    /// Transpiles to SQL
    Compile(CommandIO),
}

#[derive(clap::Args, Default)]
pub struct CommandIO {
    #[clap(default_value="-", parse(try_from_os_str = Input::try_from))]
    input: Input,

    #[clap(default_value = "-", parse(try_from_os_str = Output::try_from))]
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
                print!("{:}", error::format_error(e, &source_id, &source, true).0);
                std::process::exit(1)
            }
        }

        Ok(())
    }

    fn execute(&self, source: &str) -> Result<Vec<u8>> {
        Ok(match self {
            Cli::Parse(_) => {
                let ast = parse(source)?;

                serde_yaml::to_string(&ast)?.into_bytes()
            }
            Cli::Format(_) => crate::format(source)?.as_bytes().to_vec(),
            Cli::Debug(_) => {
                let stmts = parse(source)?;
                let (stmts, context) = semantic::resolve_only(stmts, None)?;

                let (references, stmts) =
                    semantic::label_references(stmts, &context, "".to_string(), source.to_string());

                [
                    references,
                    format!("\n{context:?}").into_bytes(),
                    serde_yaml::to_string(&stmts)?.into_bytes(),
                ]
                .concat()
            }
            Cli::Annotate(_) => {
                let stmts = parse(source)?;

                // resolve
                let (stmts, context) = semantic::resolve_only(stmts, None)?;

                let frames = semantic::collect_frames(stmts);

                // combine with source
                combine_prql_and_frames(source, frames, context)
                    .as_bytes()
                    .to_vec()
            }
            Cli::Resolve(_) => {
                let ast = parse(source)?;
                let (ir, _) = semantic::resolve(ast, None)?;

                serde_json::to_string_pretty(&ir)?.into_bytes()
            }
            Cli::Compile(_) => crate::compile(source)?.as_bytes().to_vec(),
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

fn combine_prql_and_frames(source: &str, frames: Vec<(Span, Frame)>, context: Context) -> String {
    let source = Source::from(source);
    let lines = source.lines().collect_vec();
    let width = lines.iter().map(|l| l.len()).max().unwrap_or(0);

    let mut printed_lines = 0;
    let mut result = Vec::new();
    for (span, frame) in frames {
        let line = source.get_line_range(&Range::from(span)).start;

        while printed_lines < line {
            result.push(lines[printed_lines].chars().collect());
            printed_lines += 1;
        }

        if printed_lines >= lines.len() {
            break;
        }
        let chars: String = lines[printed_lines].chars().collect();
        printed_lines += 1;

        let cols = context
            .get_column_names(&frame)
            .into_iter()
            .map(|c| c.unwrap_or_else(|| "?".to_string()))
            .join(", ");
        result.push(format!("{chars:width$} # [{cols}]"));
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
    #[should_panic]
    fn prql_layouts_test() {
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
        @"");
    }

    #[test]
    fn format_test() {
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
        from `table.subdivision`
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
