use std::path::PathBuf;
use std::process::Command;
use std::sync::Once;

static BUILD_PRQLC: Once = Once::new();

/// Return a `Command` that runs the `prqlc` binary with color/backtrace
/// stripped so snapshot tests are deterministic.
///
/// When `CARGO_BIN_EXE_prqlc` is set (integration tests), it uses that path.
/// Otherwise it locates the binary relative to the test executable and builds
/// it if necessary — `cargo test` for a bin-only crate does not produce the
/// non-test binary automatically.
pub fn prqlc_command() -> Command {
    let bin = prqlc_bin_path();
    let mut cmd = Command::new(bin);
    normalize_prqlc(&mut cmd);
    cmd
}

fn prqlc_bin_path() -> PathBuf {
    if let Some(bin) = std::env::var_os("CARGO_BIN_EXE_prqlc") {
        return PathBuf::from(bin);
    }

    // Locate the target directory from the test binary path.
    let test_bin = std::env::current_exe().expect("cannot determine test binary path");
    let mut dir = test_bin.parent().unwrap().to_path_buf();
    if dir.ends_with("deps") {
        dir.pop();
    }

    let bin_name = if cfg!(windows) { "prqlc.exe" } else { "prqlc" };
    let bin_path = dir.join(bin_name);

    // `cargo test` for a [[bin]]-only crate builds the test harness (in deps/)
    // but does NOT build the actual binary. Build it on demand if missing.
    if !bin_path.exists() {
        BUILD_PRQLC.call_once(|| {
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
            let status = Command::new(cargo)
                .args(["build", "--bin", "prqlc"])
                .status()
                .expect("failed to run `cargo build --bin prqlc`");
            assert!(status.success(), "failed to build prqlc binary");
        });
    }

    bin_path
}

fn normalize_prqlc(cmd: &mut Command) -> &mut Command {
    cmd
        // We set `CLICOLOR_FORCE` in CI to force color output, but we don't want `prqlc` to
        // output color for our snapshot tests. And it seems to override the
        // `--color=never` flag.
        .env_remove("CLICOLOR_FORCE")
        .env("NO_COLOR", "1")
        .args(["--color=never"])
        // We don't want the tests to be affected by the user's `RUST_BACKTRACE` setting.
        .env_remove("RUST_BACKTRACE")
        .env_remove("RUST_LOG")
}
