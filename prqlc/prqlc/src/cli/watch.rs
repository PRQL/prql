use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use clap::Parser;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use walkdir::WalkDir;

use super::jinja;

#[derive(Parser, Debug, Clone)]
pub struct WatchArgs {
    /// Directory or file to watch for changes
    pub path: OsString,

    #[arg(long, default_value_t = false)]
    pub no_format: bool,

    #[arg(long, default_value_t = false)]
    pub no_signature: bool,
}

pub fn run(command: &mut WatchArgs) -> Result<()> {
    let opt = prqlc::Options {
        format: !command.no_format,
        signature_comment: !command.no_signature,
        ..Default::default()
    };
    let path = Path::new(&command.path);

    // initial compile
    find_and_compile(path, &opt)?;

    // watch and compile
    println!("Watching path \"{}\"", path.display());
    watch_and_compile(path, &opt)?;

    Ok(())
}

fn find_and_compile(path: &Path, opt: &prqlc::Options) -> Result<()> {
    for entry in WalkDir::new(path) {
        // A file that doesn't compile is the ordinary starting state for
        // `watch` — aborting here would leave the errors unwatched, so we
        // report and carry on, as the watch loop below does. `compile_path`
        // has already printed the compiler's diagnostics.
        let _ignore = compile_path(entry?.path(), opt);
    }

    Ok(())
}

fn watch_and_compile(path: &Path, opt: &prqlc::Options) -> Result<()> {
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
                | notify::EventKind::Create(
                    notify::event::CreateKind::File
                    | notify::event::CreateKind::Any
                    | notify::event::CreateKind::Other,
                )
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
            Err(e) => println!("watch error: {e:?}"),
        }
    }

    Ok(())
}

fn compile_path(path: &Path, opt: &prqlc::Options) -> Result<()> {
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

    // pre-process Jinja
    let (prql_string, jinja_context) = jinja::pre_process(&prql_string)?;

    // compile
    println!("Compiling {}", prql_path.display());
    let sql_string = match prqlc::compile(&prql_string, opt) {
        Ok(sql_string) => sql_string,
        Err(errs) => {
            for err in errs.inner {
                println!("{err}");
            }
            return Err(anyhow!("failed to compile"));
        }
    };

    // post-process Jinja
    let sql_string = jinja::post_process(&sql_string, jinja_context);

    // write
    fs::write(sql_path, sql_string)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    /// A file that fails to compile used to abort the initial pass, so
    /// `prqlc watch` exited before printing "Watching path" — the one state
    /// watch mode exists to iterate out of. The other files must still compile,
    /// and the walk must reach the end.
    #[test]
    fn initial_compile_continues_past_a_failing_file() {
        let dir = TempDir::new().unwrap();
        // Named so that the failing file is walked before the second good one.
        fs::write(dir.path().join("a_good.prql"), "from tracks\n").unwrap();
        fs::write(dir.path().join("b_bad.prql"), "from tracks | filter\n").unwrap();
        fs::write(dir.path().join("c_good.prql"), "from albums\n").unwrap();

        find_and_compile(dir.path(), &prqlc::Options::default()).unwrap();

        assert!(dir.path().join("a_good.sql").is_file());
        assert!(dir.path().join("c_good.sql").is_file());
        assert!(!dir.path().join("b_bad.sql").exists());
    }
}
