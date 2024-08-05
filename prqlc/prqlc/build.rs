use std::error::Error;
// gix failing on https://github.com/rustyhorde/vergen/issues/359, and `git2`
// fails on `aarch64` so we're using `gitcl`. Switch to `gitx` when that bug is
// fixed.
use vergen_gitcl::{Emitter, GitclBuilder as GitBuilder};

pub fn main() -> Result<(), Box<dyn Error>> {
    let git = GitBuilder::default().describe(true, true, None).build()?;
    Emitter::default().add_instructions(&git)?.emit()?;
    Ok(())
}
