// From https://pyo3.rs/v0.14.5/building_and_distribution.html#macos
// Note the alternative static option with `config.toml` has an problem in https://github.com/PRQL/prql/issues/411.

use std::path::{Path, PathBuf};

fn main() {
    // Emitting any `rerun-if-changed` replaces cargo's default of re-running on
    // every change within the package, so name both inputs. That is the whole
    // set: `add_extension_module_link_args` branches only on the target OS,
    // which is part of the build fingerprint already.
    println!("cargo:rerun-if-changed=build.rs");
    let lockfile = workspace_root().join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", lockfile.display());

    let lock = std::fs::read_to_string(&lockfile)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", lockfile.display()));
    assert_pyo3_versions_aligned(&lock);

    pyo3_build_config::add_extension_module_link_args();
}

fn workspace_root() -> PathBuf {
    // `prqlc/bindings/prqlc-python` — three levels below the workspace root.
    // The crate is `publish = false`, so it is only ever built in-workspace.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("manifest dir is nested under the workspace root")
        .to_path_buf()
}

/// `pyo3-build-config` is PyO3's own build-script helper, released in lockstep
/// with `pyo3` and expected to be on the same version. Dependabot tracks the two
/// as independent entries, so merging one bump without the other silently splits
/// them: #6258 took `pyo3-build-config` to 0.29.2 while `pyo3` stayed on 0.27,
/// leaving two copies of the crate in the lockfile and building the extension
/// module's link args with a helper two minor versions ahead of the `pyo3` it
/// links against. A `pyo3` group in `.github/dependabot.yaml` keeps the bumps
/// together; this guard catches a split that arrives some other way.
fn assert_pyo3_versions_aligned(lock: &str) {
    let pyo3 = locked_versions(lock, "pyo3");
    let build_config = locked_versions(lock, "pyo3-build-config");

    assert_eq!(
        minor_versions(&pyo3),
        minor_versions(&build_config),
        "`pyo3` {pyo3:?} and `pyo3-build-config` {build_config:?} must be on the \
         same version — bump both together in the workspace `Cargo.toml`"
    );
}

/// Every version of `name` present in `Cargo.lock`, in file order.
fn locked_versions(lock: &str, name: &str) -> Vec<String> {
    let mut versions = Vec::new();
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line.trim() != format!("name = \"{name}\"") {
            continue;
        }
        let version = lines
            .next()
            .and_then(|l| l.trim().strip_prefix("version = \""))
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or_else(|| panic!("no version follows the `{name}` entry in Cargo.lock"));
        versions.push(version.to_string());
    }
    assert!(!versions.is_empty(), "`{name}` is missing from Cargo.lock");
    versions
}

/// `major.minor` of each version — PyO3 pairs the crates per minor release, so
/// a differing patch level between them is not a mismatch.
fn minor_versions(versions: &[String]) -> Vec<String> {
    versions
        .iter()
        .map(|v| {
            v.rsplit_once('.')
                .map_or(v.clone(), |(head, _)| head.to_string())
        })
        .collect()
}
