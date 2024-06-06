use std::fmt::Error;
use std::process::Command;
// use vergen::EmitBuilder;

pub fn main() -> Result<(), Error> {
    let output = Command::new("git")
        .args(["describe", "--tags"])
        .output()
        .unwrap();
    let git_hash = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=GIT_DESCRIBE={}", git_hash);

    Ok(())
}
