use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use walkdir::WalkDir;

use crate::project::ProjectTree;

pub fn read_project(project_path: PathBuf) -> Result<ProjectTree> {
    let source_extension = Some(OsStr::new("prql"));
    let data_extensions = HashSet::from([Some(OsStr::new("parquet"))]);

    let mut project = ProjectTree {
        path: project_path,
        ..Default::default()
    };

    for entry in WalkDir::new(&project.path) {
        let entry = entry?;
        let path = entry.path();
        let relative_path = path.strip_prefix(&project.path).unwrap().to_path_buf();

        if path.is_file() {
            match path.extension() {
                e if e == source_extension => {
                    let file_contents = fs::read_to_string(path)?;

                    project.sources.insert(relative_path, file_contents);
                }
                e if data_extensions.contains(&e) => {
                    project.data.insert(relative_path);
                }

                // ignore
                _ => {}
            }
        }
    }

    Ok(project)
}
