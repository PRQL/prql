use std::fmt::Error;
use std::process::Command;
// use vergen::EmitBuilder;

pub fn main() -> Result<(), Error> {
    // NOTE: This will output everything, and requires all features enabled.
    // NOTE: See the EmitBuilder documentation for configuration options.
    // EmitBuilder::builder()
    //     .git_describe(false, true, None)
    //     .emit()
    //     .unwrap();
    // note: add error checking yourself.
    let output = Command::new("git")
        .args(["describe", "--tags"])
        .output()
        .unwrap();
    let git_hash = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=GIT_DESCRIBE={}", git_hash);

    Ok(())
}
