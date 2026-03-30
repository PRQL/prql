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
    // With --target: target/<triple>/debug/deps/prqlc-<hash>
    // Without:       target/debug/deps/prqlc-<hash>
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
            let mut cmd = Command::new(cargo);
            cmd.args(["build", "--bin", "prqlc"]);

            // When tests are compiled with an explicit --target flag, artifacts
            // go into target/<triple>/debug/ instead of target/debug/. Detect
            // this and pass the same --target so the binary lands where we
            // expect it. Also handle custom --target-dir (e.g. cargo-llvm-cov).
            //
            // Path layouts we handle:
            //   target/debug/                       (default)
            //   target/<triple>/debug/              (--target)
            //   <custom-target-dir>/debug/          (--target-dir)
            //   <custom-target-dir>/<triple>/debug/ (--target + --target-dir)
            let compile_target = env!("PRQLC_BUILD_TARGET");
            if let Some(parent) = dir.parent() {
                let is_target_dir =
                    parent.file_name().and_then(|n| n.to_str()) == Some(compile_target);

                if is_target_dir {
                    cmd.args(["--target", compile_target]);
                    // The target-dir root is the grandparent.
                    if let Some(target_dir) = parent.parent() {
                        cmd.arg("--target-dir").arg(target_dir);
                    }
                } else {
                    // No explicit --target, but may be a custom --target-dir
                    // (e.g. cargo-llvm-cov uses target/llvm-cov-target/).
                    cmd.arg("--target-dir").arg(parent);
                }
            }

            let status = cmd
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
