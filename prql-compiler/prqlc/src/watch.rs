use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use clap::Parser;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use prql_compiler::downcast;
use walkdir::WalkDir;

#[derive(Parser)]
pub struct WatchCommand {
    /// Directory or file to watch for changes
    pub path: OsString,

    #[arg(long, default_value_t = false)]
    pub no_format: bool,

    #[arg(long, default_value_t = false)]
    pub no_signature: bool,
}

pub fn run(command: &mut WatchCommand) -> Result<()> {
    let opt = prql_compiler::Options {
        format: !command.no_format,
        target: prql_compiler::Target::Sql(None),
        signature_comment: !command.no_signature,
    };
    let path = Path::new(&command.path);

    // initial compile
    find_and_compile(path, &opt)?;

    // watch and compile
    println!("Watching path \"{}\"", path.display());
    watch_and_compile(path, &opt).unwrap();

    Ok(())
}

fn find_and_compile(path: &Path, opt: &prql_compiler::Options) -> Result<()> {
    for entry in WalkDir::new(path) {
        compile_path(entry?.path(), opt)?;
    }

    Ok(())
}

fn watch_and_compile(path: &Path, opt: &prql_compiler::Options) -> Result<()> {
    let cwd = std::env::current_dir().ok();

    let (tx, rx) = std::sync::mpsc::channel();

    // Automatically select the best implementation for current platform.
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(path, RecursiveMode::Recursive)?;

    for res in rx {
        match res {
            Ok(event) => match event.kind {
                notify::EventKind::Any
                | notify::EventKind::Create(notify::event::CreateKind::File)
                | notify::EventKind::Create(notify::event::CreateKind::Any)
                | notify::EventKind::Create(notify::event::CreateKind::Other)
                | notify::EventKind::Modify(_) => {
                    for path in event.paths {
                        // to make display nicer, try to convert to relative paths
                        let relative_path = if let Some(cwd) = &cwd {
                            path.strip_prefix(cwd).unwrap_or(&path)
                        } else {
                            &path
                        };

                        let _ignore = compile_path(relative_path, opt);
                    }
                }

                notify::EventKind::Access(_)
                | notify::EventKind::Create(notify::event::CreateKind::Folder)
                | notify::EventKind::Remove(_)
                | notify::EventKind::Other => {}
            },
            Err(e) => println!("watch error: {:?}", e),
        }
    }

    Ok(())
}

fn compile_path(path: &Path, opt: &prql_compiler::Options) -> Result<()> {
    // filter to only .prql files
    if path.extension() != Some(OsStr::new("prql")) {
        return Ok(());
    }

    let sql_path = path.with_extension("sql");
    let prql_path = path;

    // read
    let Some(prql_string) = fs::read_to_string(prql_path).ok() else {
        // file may not exist, because this may have been a delete event
        return Ok(());
    };
    if prql_string.is_empty() {
        return Ok(());
    }

    // compile
    println!("Compiling {}", prql_path.display());
    let sql_string = match prql_compiler::compile(&prql_string, opt.clone()) {
        Ok(sql_string) => sql_string,
        Err(err) => {
            let source_id = &prql_path.to_str().unwrap_or_default();
            print_error(anyhow!(err), source_id, &prql_string);
            return Err(anyhow!("failed to compile"));
        }
    };

    // write
    fs::write(sql_path, sql_string)?;

    Ok(())
}

fn print_error(err: anyhow::Error, source_id: &str, source: &str) {
    let messages = downcast(err).composed(source_id, source, true);

    for err in messages.inner {
        println!("{err}");
    }
}
