use std::fs;

use anyhow::Result;
use prqlc::Error;

use crate::ProjectCompiled;

/// Edit a source file such that the source of the declaration with id `decl_id` is now `new_source`.
pub fn edit_source_file(
    project: &ProjectCompiled,
    decl_id: usize,
    new_source: String,
) -> Result<()> {
    let span = project.root_module.span_map.get(&decl_id);
    let Some(span) = span else {
        // TODO: bad error message, we should not mention decl ids
        return Err(Error::new_simple(format!(
            "cannot find where declaration {decl_id} came from"
        ))
        .into());
    };

    // retrieve file path, relative to project root
    // this is safe, because the source_id must exist, right? It was created during parsing.
    let file_path = project.sources.get_path(span.source_id).unwrap().clone();

    // find original source
    let mut source = project.sources.sources.get(&file_path).unwrap().clone();

    // replace the text
    source.replace_range(span.start..span.end, &new_source);

    // reconstruct full path of the file
    // sources are loaded from file-system, so root path must always exist
    let mut file_path_full = project.sources.root.clone().unwrap();
    file_path_full.extend(&file_path);

    // write the new file contents
    fs::write(file_path_full, &source)?;
    Ok(())
}
