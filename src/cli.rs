use crate::*;
use anyhow::Error;
use clap::{ArgEnum, Args, Parser};
use clio::{Input, Output};
use std::io::{Read, Write};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum Dialect {
    Ast,
    MaterializedAst,
    Sql,
}

#[derive(Parser)]
#[clap(name = env!("CARGO_PKG_NAME"), about, version, author)]
pub enum Cli {
    Compile(CompileCommand),
}

#[derive(Args)]
#[clap()]
pub struct CompileCommand {
    #[clap(default_value="-", parse(try_from_os_str = Input::try_from))]
    input: Input,

    #[clap(short, long, default_value = "-", parse(try_from_os_str = Output::try_from))]
    output: Output,

    #[clap(short, long, arg_enum, default_value = "sql")]
    format: Dialect,
}

impl Cli {
    pub fn execute(&mut self) -> Result<(), Error> {
        match self {
            Cli::Compile(command) => {
                let mut source = String::new();
                command.input.read_to_string(&mut source)?;

                match command.format {
                    Dialect::Ast => command
                        .output
                        .write_all(&serde_yaml::to_vec(&parse(&source)?)?)?,
                    Dialect::MaterializedAst => {
                        let materialized = materialize(parse(&source)?)?;
                        command
                            .output
                            .write_all(&serde_yaml::to_vec(&materialized)?)?
                    }
                    Dialect::Sql => {
                        command.output.write_all(transpile(&source)?.as_bytes())?;
                    }
                };
            }
        }

        Ok(())
    }
}
