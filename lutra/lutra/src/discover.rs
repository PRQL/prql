use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use walkdir::WalkDir;

use crate::project::ProjectDiscovered;

#[cfg_attr(feature = "clap", derive(clap::Parser))]
pub struct DiscoverParams {
    /// Path to the project directory
    #[cfg_attr(feature = "clap", arg(default_value = "."))]
    pub project_path: PathBuf,
}

pub fn discover(params: DiscoverParams) -> Result<ProjectDiscovered> {
    let source_extension = Some(OsStr::new("prql"));

    let mut project = ProjectDiscovered {
        root_path: params.project_path,
        ..Default::default()
    };

    for entry in WalkDir::new(&project.root_path) {
        let entry = entry?;
        let path = entry.path();
        let relative_path = path.strip_prefix(&project.root_path).unwrap().to_path_buf();

        if path.is_file() {
            match path.extension() {
                e if e == source_extension => {
                    let file_contents = fs::read_to_string(path)?;

                    project.sources.insert(relative_path, file_contents);
                }

                // ignore
                _ => {}
            }
        }
    }

    Ok(project)
}
