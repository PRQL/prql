use std::error::Error;
// gix failing on https://github.com/rustyhorde/vergen/issues/359, so we're
// using `git2`
use vergen_git2::{Emitter, Git2Builder as GitBuilder};

pub fn main() -> Result<(), Box<dyn Error>> {
    let git = GitBuilder::default().describe(true, true, None).build()?;
    Emitter::default().add_instructions(&git)?.emit()?;
    Ok(())
}
