use std::fmt::Error;
use vergen::EmitBuilder;

pub fn main() -> Result<(), Error> {
    // NOTE: This will output everything, and requires all features enabled.
    // NOTE: See the EmitBuilder documentation for configuration options.
    EmitBuilder::builder()
        .all_build()
        .all_cargo()
        .all_git()
        .all_rustc()
        .all_sysinfo()
        .emit()
        .unwrap();
    Ok(())
}
