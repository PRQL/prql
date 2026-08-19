#!/usr/bin/env bash

# Install apt packages, bounding each apt call so a stalled Ubuntu mirror
# resolves the step in minutes instead of hanging the job until GitHub's 6h
# cap.
#
# `apt-get update` has no overall wall-clock bound: a connection that opens and
# then trickles keeps it waiting indefinitely. On 2026-08-18 the
# `build-prqlc-c (ubuntu-24.04, x86_64-unknown-linux-musl)` job on `main` sat
# in `apt-get update` for 359 minutes — it had reached
# `Get:5 https://archive.ubuntu.com/ubuntu noble-security InRelease` and never
# came back — and was killed at the 360-minute default:
# https://github.com/PRQL/prql/actions/runs/32160030161
#
# A job killed at the cap reports `cancelled`, not `failure`, so it doesn't
# read as a broken build to anything watching the default branch. Bounding each
# apt call keeps that signal honest — whatever happens, the step now resolves in
# minutes with a conclusion that means what it says.
#
# The per-attempt budget has to cover the degraded-but-working case, not just
# the healthy one. `azure.archive.ubuntu.com` is regularly unreachable on these
# runners; apt spends ~40s retrying it before falling back to
# `archive.ubuntu.com`, and only then starts on the package indices. A healthy
# update takes a few seconds, so 5 minutes is generous for the slow path while
# still bounding the pathological one at ~22 minutes (three update attempts
# plus the kill grace, then the install's own budget) rather than 6 hours.
#
# `timeout` runs under `sudo` (rather than the reverse) so it signals
# `apt-get` directly. `-k 30` is what makes the budget a bound rather than a
# request: plain `timeout` sends SIGTERM and then waits for the child forever,
# and apt defers termination signals while dpkg is working, so an install that
# ignores its SIGTERM would be straight back to running out the 6h cap. `-k`
# follows up with SIGKILL 30s later, which nothing can defer.
#
# A failed `update` is a warning, not an error. The `ubuntu-24.04` image ships
# with `/var/lib/apt/lists` already populated, so the packages we install
# resolve without a working index refresh — on a stock runner,
# `apt-cache policy musl-tools` reports candidate 1.2.4-2 from
# `noble/universe`, and `apt-get install -s` for both `musl-tools` and
# `gcc-aarch64-linux-gnu` plans a complete install before any `update` runs.
# Refreshing is still worth attempting, since a stale index can 404 at download
# time, but a mirror that won't serve `InRelease` is not a reason to fail a step
# that can resolve without it. The install still fetches the `.deb` from that
# same mirror under the same bound, so a full outage is still red — only the
# install decides the step's exit status.

set -euo pipefail

updated=""
for attempt in 1 2 3; do
  if sudo timeout -k 30 300 apt-get update; then
    updated=1
    break
  fi
  echo "::warning::apt-get update stalled or failed (attempt ${attempt})"
  if [ "$attempt" -lt 3 ]; then
    sleep 10
  fi
done

if [ -z "$updated" ]; then
  echo "::warning::apt-get update did not complete after 3 attempts; installing from the package lists baked into the runner image"
fi

status=0
sudo timeout -k 30 300 apt-get install -y "$@" || status=$?
if [ "$status" -ne 0 ]; then
  echo "::error::apt-get install failed (exit ${status}) for: $*"
  exit "$status"
fi
