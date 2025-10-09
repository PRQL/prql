# macOS Symbol Length Issues with Chumsky 0.11.1

## Problem

Upgrading from Chumsky 0.9/0.10 to 0.11.1 causes macOS linker failures due to deeply nested generic types creating symbol names that exceed macOS ld64's hard length limits:

```
ld: Assertion failed: (name.size() <= maxLength), function makeSymbolStringInPlace, file SymbolString.cpp, line 74.
clang: error: linker command failed with exit code 1
```

## Root Cause

Chumsky's parser combinator types create deeply nested generic signatures. Each combinator adds another layer of type parameters, resulting in extremely long mangled symbol names. The macOS system linker (ld64) has a hard-coded maximum symbol length that these types exceed.

## Constraint: Python Bindings

Python bindings (via maturin/PyO3) require macOS-specific linker flags:
- `-undefined dynamic_lookup` - Required for Python extension modules
- These flags were historically thought incompatible with rust-lld

## Research Findings

### rust-lld Compatibility Discovery

**Critical finding**: rust-lld on macOS uses `ld64.lld` backend, which **does support** `-undefined dynamic_lookup`:

1. **LLVM D93263 (2020)**: Implemented `-undefined TREATMENT` including `dynamic_lookup`
2. **LLVM D97521 (2021)**: Added support for dynamic lookup symbols
3. **LLVM D106565 (2021)**: Fixed interaction with `-dead_strip`

This means rust-lld should work for Python bindings despite earlier assumptions.

### How Symbol Length Solutions Compare

| Solution | Consistency | Maintainability | Effectiveness | Complexity |
|----------|-------------|-----------------|---------------|------------|
| rust-lld globally | ✅ Single config | ✅ One line | ✅ Proven | ✅ Simple |
| v0 mangling globally | ✅ Single config | ✅ Profile setting | ⚠️ May be insufficient | ✅ Simple |
| Per-workflow env vars | ❌ CI-specific | ❌ Multiple files | ✅ Works | ❌ Complex |
| Strategic boxing | ✅ Code-level | ⚠️ Ongoing | ⚠️ Partial | ⚠️ Moderate |
| Build scripts | ⚠️ Per-crate | ⚠️ Complex | ✅ Flexible | ❌ Complex |

## Research Update: rust-lld Limitations

**Finding**: rust-lld does NOT work with Python bindings despite ld64.lld's theoretical support for `-undefined dynamic_lookup`. The issue is that maturin passes linker flags with the `-Wl,` prefix:

```
-Wl,-install_name,@rpath/prqlc.abi3.so
```

rust-lld doesn't understand the `-Wl,` prefix and fails with:
```
rust-lld: error: unknown argument '-Wl,-install_name,@rpath/prqlc.abi3.so'
```

The system linker (ld64) strips the `-Wl,` prefix and processes the flag correctly. This is a fundamental incompatibility.

## Problem with Simple Solutions

**Cargo config hierarchy issue**: The `.cargo/config.toml` linker setting cannot be overridden by:
- Package-specific build.rs (arguments get passed to rust-lld, not intercepted)
- Package-specific cargo configs (don't exist in cargo's design)
- Profile settings in package Cargo.toml (cargo ignores non-root profiles)

**The only override mechanism**: `CARGO_TARGET_*_LINKER` environment variable

## Recommended Solution: Minimal Per-Workflow Config

Accept that environment variables are necessary, but minimize them:

**`.cargo/config.toml`** (rust-lld for all platforms where it works):
```toml
[target.x86_64-pc-windows-msvc]
linker = "rust-lld"

[target.aarch64-apple-darwin]
linker = "rust-lld"

[target.x86_64-apple-darwin]
linker = "rust-lld"
```

**Python bindings workflow** (`test-python.yaml`, `release.yaml`):
```yaml
env:
  # Python bindings need system linker due to maturin's -Wl, prefixed flags
  # AND v0 symbol mangling to reduce symbol lengths for ld64's hard limits
  CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER: ${{ startsWith(matrix.os, 'macos') && 'cc' || '' }}
  CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER: ${{ startsWith(matrix.os, 'macos') && 'cc' || '' }}
  RUSTFLAGS: ${{ startsWith(matrix.os, 'macos') && '-C debuginfo=0 -Csymbol-mangling-version=v0' || '-C debuginfo=0' }}
```

**All other workflows** (test-java, test-rust, release for prqlc/prqlc-c):
- No environment variables needed
- rust-lld from cargo config works automatically

### Why This Is The Least-Bad Solution

1. **Cargo limitation** - No way to override linker config per-package without environment variables
2. **Minimized scope** - Only Python bindings need special config (not all workflows)
3. **Clear separation** - Python = system linker + v0, everything else = rust-lld
4. **Documented** - Comments explain why Python is special
5. **Testable locally** - Set same env vars to reproduce CI behavior

### Testing Required

```bash
# 1. Clean build
cargo clean

# 2. Test regular compilation
cargo build

# 3. Test Python bindings
cd prqlc/bindings/prqlc-python
maturin build
pip install --force-reinstall target/wheels/*.whl
python -c "import prqlc; print(prqlc.compile('from x | select y'))"

# 4. Run full test suite
cargo test --all
```

### Fallback if Python Bindings Fail

If rust-lld proves incompatible with Python bindings (unlikely but possible):

**Option A: v0 Symbol Mangling Globally**
```toml
# Cargo.toml (workspace level)
[profile.dev]
symbol-mangling-version = "v0"

[profile.release]
symbol-mangling-version = "v0"
```

**Option B: Build Script Detection**
```rust
// prqlc/bindings/prqlc-python/build.rs
fn main() {
    #[cfg(target_os = "macos")]
    {
        // Force system linker for Python bindings only
        println!("cargo:rustc-link-arg=-fuse-ld=ld");
    }
}
```

## Current State (Problematic)

The codebase currently uses per-workflow environment variables:

```yaml
# Different configs in different workflows
env:
  CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER: rust-lld  # In some workflows
  RUSTFLAGS: "-Csymbol-mangling-version=v0"           # In others
```

**Problems**:
- ❌ CI runs under different configuration than local dev
- ❌ Multiple files to maintain (6 workflow files)
- ❌ Easy to miss when adding new workflows
- ❌ Harder to debug ("why does it work in CI but not locally?")
- ❌ Divergence between environments creates risk

## Action Plan

1. **Remove all per-workflow environment variables** for linker configuration
2. **Add rust-lld to `.cargo/config.toml`** for all macOS targets
3. **Test Python bindings thoroughly** to verify compatibility
4. **If issues arise**: Fall back to v0 mangling or build script approach
5. **Document the solution** in cargo config with clear comments

## References

- Leptos Issue #3148: Same error, solved with rust-lld
- Rust Issue #141626: Windows symbol length issues
- LLVM Reviews: D93263, D97521, D106565 (ld64.lld `-undefined dynamic_lookup` support)
- Chumsky Issue #13: Type complexity and compilation performance

## Performance Impact

The Chumsky 0.11.1 upgrade delivers **7x faster parsing** (85-87% reduction in parse time) across all benchmarks. The symbol length issue is the final blocker to landing these performance improvements.
