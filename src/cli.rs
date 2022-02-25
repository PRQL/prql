use anyhow::Error;
use clap::{ArgEnum, Parser};
use core::fmt;
use std::{
    ffi::{OsStr, OsString},
    fmt::Display,
    fs::{File, OpenOptions},
    io::{self, Read, Stdin, Stdout, Write},
    usize,
};

#[cfg(not(unix))]
fn is_fifo(file: &File) -> Result<bool, Error> {
    Ok(false)
}

#[cfg(unix)]
fn is_fifo(file: &File) -> Result<bool, Error> {
    use std::os::unix::fs::FileTypeExt;
    Ok(file.metadata()?.file_type().is_fifo())
}

#[derive(Debug)]
pub enum Input {
    Stdin(Stdin),
    Pipe(OsString, File),
    File(OsString, File),
}

#[derive(Debug)]
pub enum Output {
    Stdout(Stdout),
    Pipe(OsString, File),
    File(OsString, File),
}

impl Input {
    pub fn new<S: AsRef<OsStr>>(path: S) -> Result<Self, Error> {
        let path = path.as_ref();
        if path == "-" {
            Ok(Input::Stdin(io::stdin()))
        } else {
            let file = File::open(path)?;
            if is_fifo(&file)? {
                Ok(Input::Pipe(path.to_os_string(), file))
            } else {
                Ok(Input::File(path.to_os_string(), file))
            }
        }
    }

    pub fn try_from_os_str(path: &OsStr) -> std::result::Result<Self, String> {
        Input::new(path).map_err(|e| e.to_string())
    }
}

impl Output {
    pub fn new<S: AsRef<OsStr>>(path: S) -> Result<Self, Error> {
        let path = path.as_ref();
        if path == "-" {
            Ok(Output::Stdout(io::stdout()))
        } else {
            let file = open_rw(path)?;
            if is_fifo(&file)? {
                Ok(Output::Pipe(path.to_os_string(), file))
            } else {
                Ok(Output::File(path.to_os_string(), file))
            }
        }
    }

    pub fn try_from_os_str(path: &OsStr) -> std::result::Result<Self, String> {
        Output::new(path).map_err(|e| e.to_string())
    }
}

fn open_rw(path: &OsStr) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
}

impl Read for Input {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        match self {
            Input::Stdin(stdin) => stdin.read(buf),
            Input::Pipe(_, pipe) => pipe.read(buf),
            Input::File(_, file) => file.read(buf),
        }
    }
}

impl Write for Output {
    fn flush(&mut self) -> Result<(), io::Error> {
        match self {
            Output::Stdout(stdout) => stdout.flush(),
            Output::Pipe(_, pipe) => pipe.flush(),
            Output::File(_, file) => file.flush(),
        }
    }
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
        match self {
            Output::Stdout(stdout) => stdout.write(buf),
            Output::Pipe(_, pipe) => pipe.write(buf),
            Output::File(_, file) => file.write(buf),
        }
    }
}

impl Display for Output {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Output::Stdout(_) => write!(fmt, "-"),
            Output::Pipe(path, _) => write!(fmt, "{:?}", path),
            Output::File(path, _) => write!(fmt, "{:?}", path),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum)]
enum Dialect {
    Sql,
}

#[derive(Parser)]
#[clap(name = env!("CARGO_PKG_NAME"), about, version, author)]
pub struct Cli {
    #[clap(parse(try_from_os_str = Input::try_from_os_str), help("Name of file (use '-' to use stdin instead)"))]
    input: Input,

    #[clap(short, long, default_value = "-", parse(try_from_os_str = Output::try_from_os_str))]
    output: Output,

    #[clap(short, long, arg_enum, default_value = "sql")]
    format: Format,
}

impl Cli {
    pub fn execute(&mut self) -> Result<(), Error> {
        let mut source = String::new();
        self.input.read_to_string(&mut source)?;
        let output = match self.format {
            Format::Sql => {
                format!("TODO! do something useful with source:\n{}", source)
            }
        };
        self.output.write_all(output.as_bytes())?;
        Ok(())
    }
}
