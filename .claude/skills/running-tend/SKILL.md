---
name: running-tend
description:
  PRQL-specific guidance for tend CI workflows. Adds a standing exception for
  filing issues in other repos, PR title conventions, CI structure, which test
  commands actually run inside the sandbox, Dependabot-batch polling, weekly
  maintenance tasks, and issue-closing policy on top of the generic tend-*
  skills. Use when operating in CI.
---

# Running Tend in PRQL

Tend-specific guidance for this repo. Project build commands, test strategy,
error conventions, etc. are in `CLAUDE.md` — don't duplicate them here.

## Filing issues in other repos

Standing exception granted: file directly in agent-equipped targets (per
**Filing Issues in Other Repos** in the bundled `running-in-ci` skill) without
asking permission here first. The default rule (open an issue here asking
permission first) still applies when the target shows no agent signals.

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

## Running tests from a tend session

**`cargo-insta` and `cargo-nextest` are not on the sandbox PATH**, so the
commands `CLAUDE.md` documents as the inner loop do not run here.
`.github/actions/tend-setup` installs both, but `baptiste0928/cargo-install`
puts them under the runner user's home — `.cargo-install/<crate>/bin` there —
and the agent runs as a separate `tend-sandbox` user whose PATH cannot carry a
runner-home path. Both subcommands answer `no such command`, which takes down
`task prqlc:test` and `task prqlc:pull-request`: both route through
`cargo insta test --accept … --test-runner=nextest`.

What to run instead:

- `cargo test` directly, scoped the same way the `CLAUDE.md` examples are — e.g.
  `cargo test -p prqlc --test integration -- date`.
- **Don't `cargo install` either crate to work around this.** Building them from
  source costs several minutes of the session budget and the binaries are thrown
  away with the runner.
- **Never verify with `INSTA_UPDATE=always cargo test`.** `always` selects
  insta's in-place update, so a `.snap` file is rewritten to whatever the code
  produced and the assertion passes unconditionally — a green run that checked
  nothing. Use it only to regenerate file snapshots deliberately, then re-run
  plain `cargo test` to verify. Plain `cargo test` is a real check: insta's
  default `auto` behaviour writes nothing when `CI` is set.
- **Inline snapshots can't be auto-accepted here.** `CLAUDE.md` asks for
  `assert_snapshot!(result, @"")` filled in by `--accept`, but insta itself
  never rewrites a source file — it records the value in a pending-snapshot file
  and leaves applying it to `cargo-insta`. Take the expected value from the test
  failure's diff, write it into the `@"…"` literal by hand, and re-run to
  confirm it matches.
- Scope the claim to the command that actually ran. `cargo test -p prqlc` is not
  `task prqlc:pull-request`, and saying so is the difference between a useful
  caveat and a false green.

**Don't re-propose the infra fix.** #6144 (symlinking both binaries into
`/usr/local/bin`) sat open for 20 days and was closed unmerged by a maintainer
on 2026-08-25, hours after
[max-sixty/tend#1048](https://github.com/max-sixty/tend/pull/1048) landed
upstream — which makes runner-home setup explicit and names `sandbox_setup:`
(with `sandbox_path:`) as the supported lever for sandbox-scoped installs.
Whether to pull that lever in `.config/tend.yaml` is a maintainer's call, not
something to re-litigate from a session.

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
(17:14:37 → 18:27:46), far past the 9-minute cap on the poll loop in **CI
Monitoring** in `running-in-ci`.

**Stop after one poll round when every pending check is `QUEUED`.** A `QUEUED`
check has not been allocated a runner, so another round changes nothing: post
the verdict, name the unverified checks, and end. If any pending check is
`IN_PROGRESS`, keep polling — that work is advancing and may still settle.

The `pending()` helper in **CI Monitoring** returns a count without the states,
so it can't tell those two cases apart. Project the states alongside it:

```sh
gh pr view <n> --json statusCheckRollup \
  --jq '[.statusCheckRollup[] | {name: (.name // .context), status: (.status // .state)}]'
```

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
