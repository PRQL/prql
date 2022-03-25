// TODO:
// - It would be a bit nicer to have the script be in rust; so it didn't
//   depend on hoping we get a recently compiled version in `./target/debug` /
//   could run on any platform.
// - Currently it mixes "not yet implemented" with "an actual error". We should
//   really disaggregate those; e.g. add a comment of "not yet implemented" for
//   those that error, rather than just ignoring them.

use std::str;
#[test]
fn run_examples() {
    if cfg!(not(target_os = "windows")) {
        use std::process::Command;
        let output = Command::new("./examples/prql/generate-md.sh")
            .output()
            .unwrap();
        match output.status.code() {
            Some(0) => {
                assert_eq!(output.status.code(), Some(0));
            }
            _ => panic!(
                "Unexpected exit status: {:?}\n {:?} \n{:?}",
                output.status,
                str::from_utf8(&output.stdout),
                str::from_utf8(&output.stderr)
            ),
        }
    }
}
