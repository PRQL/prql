#!/usr/bin/env bash

# Install apt packages, bounding each apt call so a stalled Ubuntu mirror fails
# the step in minutes instead of hanging the job until GitHub's 6h cap.
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
# read as a broken build to anything watching the default branch. Failing fast
# keeps that signal honest, and a transient mirror stall is retried rather than
# waiting out the cap.
#
# The per-attempt budget has to cover the degraded-but-working case, not just
# the healthy one. `azure.archive.ubuntu.com` is regularly unreachable on these
# runners; apt spends ~40s retrying it before falling back to
# `archive.ubuntu.com`, and only then starts on the package indices. A healthy
# update takes a few seconds, so 5 minutes is generous for the slow path while
# still bounding the pathological one at ~15 minutes rather than 6 hours.
#
# `timeout` runs under `sudo` (rather than the reverse) so it signals
# `apt-get` directly.

set -euo pipefail

for attempt in 1 2 3; do
  if sudo timeout 300 apt-get update; then
    break
  fi
  if [ "$attempt" -eq 3 ]; then
    echo "::error::apt-get update did not complete after 3 attempts"
    exit 1
  fi
  echo "::warning::apt-get update stalled or failed (attempt ${attempt}); retrying"
  sleep 10
done

sudo timeout 300 apt-get install -y "$@"
