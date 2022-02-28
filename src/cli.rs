use crate::parser::{ast_of_string, Rule};
use anyhow::Error;
use clap::{ArgEnum, Parser};
use clio::{Input, Output};
use std::io::{Read, Write};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum Dialect {
    Ast,
}

#[derive(Parser)]
#[clap(name = env!("CARGO_PKG_NAME"), about, version, author)]
pub struct Cli {
    #[clap(default_value="-", parse(try_from_os_str = Input::try_from))]
    input: Input,

    #[clap(short, long, default_value = "-", parse(try_from_os_str = Output::try_from))]
    output: Output,

    #[clap(short, long, arg_enum, default_value = "ast")]
    format: Dialect,
}

impl Cli {
    pub fn execute(&mut self) -> Result<(), Error> {
        let mut source = String::new();
        self.input.read_to_string(&mut source)?;
        match self.format {
            Dialect::Ast => self
                .output
                .write_all(&serde_yaml::to_vec(&ast_of_string(&source, Rule::query)?)?)?,
        };
        Ok(())
    }
}
