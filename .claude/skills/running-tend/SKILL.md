---
name: running-tend
description:
  PRQL-specific guidance for tend CI workflows. Adds a standing exception for
  filing issues in other repos, PR title conventions, CI structure,
  Dependabot-batch polling, weekly maintenance tasks, and issue-closing policy
  on top of the bundled tend-ci-runner skills. Use when operating in CI.
---

# Running Tend in PRQL

Tend-specific guidance for this repo. Project build commands, test strategy,
error conventions, etc. are in `CLAUDE.md` — don't duplicate them here.

## Filing issues in other repos

Standing exception granted: file directly in agent-equipped targets (per
**Filing issues** in the bundled `/tend-ci-runner:act-in-other-repos` skill)
without asking permission here first. The default rule (open an issue here
asking permission first) still applies when the target shows no agent signals.

## PR conventions

- PR titles use conventional commits: `feat:`, `fix:`, `docs:`, `chore:`,
  `refactor:`, `test:`, `ci:`, `internal:`, `devops:`, `web:`, `refine:`
- No scope required (e.g., `fix: resolve date parsing` not `fix(parser): ...`)
- Dependabot PRs use `chore:` prefix

## CI structure

- Main CI workflow: `tests` (watched by tend-ci-fix)
- Dependency management: Dependabot opens dependency PRs; tend-weekly reviews
  them and runs the tasks under Weekly maintenance below.
- **tend's own action is excluded from Dependabot** (`max-sixty/tend` is in the
  github-actions `ignore` list in `.github/dependabot.yaml`). Tend updates flow
  through the nightly `tend/update-workflows` regen (`uvx tend init`), which
  follows structural changes a version-only bump can't — e.g. the 0.1.7 move
  that split the action into `claude/`/`codex/` subdirectories broke the naive
  Dependabot bump #6031. Don't re-add `max-sixty/tend` to Dependabot.
- Automerge: not configured — `pull-request-target.yaml` only validates PR
  titles and handles `pr-backport-web` backports. The automerge job was removed
  in #5753, so bot PRs must be merged manually by a maintainer (or via repo
  branch-protection auto-merge if a maintainer enables it on the PR).

## Verifying a `rust-toolchain.toml` bump

The `update-rust-toolchain` action opens `build: Update rust toolchain version`
with nobody owning it, so tend usually pushes the mechanical lint fixes a new
clippy demands (the repo compiles with `-D warnings`). **Verify with the
matrix's full feature set, not `--features=default`.** The `tests` matrix runs
clippy as
`--all-targets --no-default-features --features=default,test-dbs-external,lsp`,
and code behind `test-dbs-external` — `prqlc/prqlc/tests/integration/dbs/` — is
invisible to a `default`-only run.

On #6219 (1.96.1 → 1.97.1) a session fixed the 7 `useless_borrows_in_formatting`
sites `--features=default` exposed and posted "clean across the workspace", but
an 8th in `dbs/runner.rs` was still red — so the posted claim was wrong, not
just the branch.

A whole-workspace `--all-targets` clippy on a cold cache exceeds the session
budget. Scope it to the failing compilation unit instead (~9 minutes):

```sh
cargo clippy -p prqlc --test integration --target=x86_64-unknown-linux-gnu \
  --no-default-features --features=default,test-dbs-external,lsp -- -D warnings
```

Then scope the resulting claim to match the command: a clean run there clears
that one compilation unit, not the workspace — other crates and targets stay
unchecked. Name the unit that was verified rather than repeating #6219's "clean
across the workspace".

## CI polling during the Dependabot batch

Dependabot opens its whole batch over a couple of minutes (the 2026-08-03 batch
ran 17:14:32 → 17:16:45; across 2026-06 to 2026-08 every batch has landed in
17:12–17:19 UTC), so five or six `tests` matrices compete for runners at once.
The surviving `tests` run on each PR then sits in `QUEUED` for a long time
before it starts — run `30835855220` on #6130 took 73 minutes end to end
(17:14:37 → 18:27:46), far past the 9-minute cap on the poll loop in
`/tend-ci-runner:monitor-ci`.

**Stop after one poll round when every pending check is `QUEUED`.** A `QUEUED`
check has not been allocated a runner, so another round changes nothing: post
the verdict, name the unverified checks, and end. If any pending check is
`IN_PROGRESS`, keep polling — that work is advancing and may still settle.

`poll_pr_checks.py` prints the still-pending checks by name without their
states, so it can't tell those two cases apart. Project the states alongside it:

```sh
gh pr view <n> --json statusCheckRollup \
  --jq '[.statusCheckRollup[] | {name: (.name // .context), status: (.status // .state)}]'
```

## Verifying a change to the .NET binding

`prqlc/bindings/dotnet/` builds and tests from a session, but two sandbox
constraints block it first and neither names its own cause.

**`/tmp` is read-only, and the .NET SDK insists on it.** The SDK's first-run
configurer creates a named `Mutex` whose backing directory it places under a
hard-coded `/tmp`, so `dotnet build`, `restore`, `test` and `new` all abort with
`mkdtemp("/tmp/.dotnet.XXXXXX") == nullptr; errno == EROFS`. No environment
variable moves that path — `TMPDIR`, `DOTNET_CLI_HOME`, `NUGET_PACKAGES` and
`DOTNET_SKIP_FIRST_TIME_EXPERIENCE` all leave it alone — and `dotnet --info` and
`dotnet --version` succeed because neither reaches the configurer, so the SDK
looks usable right up to the first build. Run `dotnet` through
`scripts/with-writable-tmp.sh`, which gives the command a private tmpfs at
`/tmp`.

**MSBuild's worker nodes can't start, and say nothing.** Creating an `AF_UNIX`
socket is refused sandbox-wide with `EPERM` — `socketpair` still works, so
ordinary parent/child pipes are unaffected and only cross-process Unix-socket
IPC is lost. A multi-node build dies in the node handshake and reports
`Build FAILED.` with `0 Error(s)` and no diagnostic at all. **Pass `-m:1` to
every `dotnet` command** — without it a real failure is indistinguishable from
this one. A single-project build succeeds either way, which is why a quick probe
misses it.

Together they reproduce the whole `test-dotnet` job from the repo root:

```sh
cargo build -p prqlc-c
W=.claude/skills/running-tend/scripts/with-writable-tmp.sh
$W dotnet build prqlc/bindings/dotnet -m:1
cp target/debug/libprqlc_c.* prqlc/bindings/dotnet/PrqlCompiler/bin/Debug/net*/
cp target/debug/libprqlc_c.* prqlc/bindings/dotnet/PrqlCompiler.Tests/bin/Debug/net*/
$W dotnet test prqlc/bindings/dotnet -m:1
```

Restore prints `NU1903` for `Newtonsoft.Json` 9.0.1, pulled in transitively by
`Microsoft.NET.Test.Sdk`. It predates any change under review — don't chase it.

**The wrapper looks unnecessary after the first wrapped run, and isn't.** That
run completes the SDK's first-use configuration and writes
`~/.dotnet/<version>.dotnetFirstUseSentinel` into the real home, which persists
outside the tmpfs — so an unwrapped `dotnet` afterwards gets past the configurer
and looks fixed. It isn't: NuGet's `MigrationRunner` takes the same mutex again
inside `RestoreTask`, and the same `mkdtemp` failure resurfaces as an `MSB4018`
from `NuGet.targets`. Probe the constraint from a session that has run no
wrapped `dotnet` command, or not at all.

## Weekly maintenance

These tasks run as Step 3 of the bundled weekly skill (only when
`workflows.weekly` is enabled in `.config/tend.yaml`).

- **Bump pinned `go-task/setup-task` version.** The action is invoked with a
  concrete `version:` input to avoid the intermittent
  `unable to get latest version` failure from `version: 3.x` (see #5836).
  Dependabot does not update `with:` inputs, so this needs a manual weekly bump.
  Find the latest release at <https://github.com/go-task/task/releases/latest>;
  if the current pin is older, update `version: X.Y.Z` in:
  - `.github/actions/tend-setup/action.yaml`
  - `.github/workflows/build-web.yaml`
  - `.github/workflows/test-php.yaml`
  - `.github/workflows/test-prqlc-c.yaml`

  Open a single `chore:` PR with the bump. Skip if already at the latest.

## Issue management

- Close bot-opened issues once the underlying cause is resolved — don't leave
  them open for a maintainer. If you (prql-bot) filed an issue (e.g., a nightly
  "tests failed" issue, a code-quality issue, an infra/upstream bug report) and
  the fix has merged or the upstream problem has been addressed, close the issue
  with a short comment citing the resolution (e.g., "Resolved by #NNNN —
  closing"). Applies to any issue where `author.login == prql-bot`.
