use crate::{materialize, parse, reporting, translate};
use anyhow::Error;
use clap::{ArgEnum, Args, Parser};
use clio::{Input, Output};
use std::io::{Read, Write};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum Format {
    Ast,
    MaterializedAst,
    Sql,
}

#[derive(Parser)]
#[clap(name = env!("CARGO_PKG_NAME"), about, version)]
pub enum Cli {
    Compile(CompileCommand),
}

#[derive(Args)]
/// Compile a PRQL string into a SQL string.
///
/// See https://github.com/max-sixty/prql for more information.
pub struct CompileCommand {
    #[clap(default_value="-", parse(try_from_os_str = Input::try_from))]
    input: Input,

    #[clap(short, long, default_value = "-", parse(try_from_os_str = Output::try_from))]
    output: Output,

    #[clap(short, long, arg_enum, default_value = "sql")]
    format: Format,
}

fn is_stdin(input: &Input) -> bool {
    input.path() == "-"
}

impl Cli {
    pub fn execute(&mut self) -> Result<(), Error> {
        match self {
            Cli::Compile(command) => {
                // Don't wait without a prompt when running `prql compile` —
                // it's confusing whether it's waiting for input or not. This
                // offers the prompt.
                if is_stdin(&command.input) && atty::is(atty::Stream::Stdin) {
                    println!("Enter PRQL, then ctrl-d:");
                    println!();
                }

                let mut source = String::new();
                command.input.read_to_string(&mut source)?;
                let source_id = command.input.path().clone().to_str().unwrap();

                let res = compile_to(command.format, &source);

                match res {
                    Ok(buf) => {
                        command.output.write(&buf)?;
                    }
                    Err(e) => {
                        reporting::print_error(e, source_id, &source)
                    }
                };
            }
        }

        Ok(())
    }
}

fn compile_to(format: Format, source: &str) -> Result<Vec<u8>, Error> {
    Ok(match format {
        Format::Ast => {
            let ast = parse(source)?;

            serde_yaml::to_vec(&ast)?
        }
        Format::MaterializedAst => {
            let materialized = materialize(parse(source)?)?;

            serde_yaml::to_vec(&materialized)?
        }
        Format::Sql => {
            let materialized = materialize(parse(source)?)?;
            let sql = translate(&materialized)?;

            sql.as_bytes().to_vec()
        }
    })
}
