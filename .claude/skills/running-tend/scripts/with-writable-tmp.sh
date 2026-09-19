#!/usr/bin/env bash
# Run a command with a writable /tmp, for tools that hard-code paths there.
#
# The tend sandbox mounts /tmp read-only, so the .NET SDK's first-run
# configurer — which creates a named Mutex under a hard-coded /tmp — aborts
# every build, restore and test with
# `mkdtemp("/tmp/.dotnet.XXXXXX") == nullptr; errno == EROFS`. No environment
# variable moves that path.
#
# A private mount namespace with a tmpfs over /tmp satisfies it. The checkout
# itself lives under /tmp, so it is bind-mounted outside /tmp first; the tmpfs
# would otherwise hide it. This grants no access the session does not already
# have: the read-only mounts stay read-only inside the namespace, and the
# tmpfs is empty and private to the command.
#
#   .claude/skills/running-tend/scripts/with-writable-tmp.sh dotnet build dotnet
#
# The command runs at the same position in the tree, reached through the bind,
# so relative paths work and writes land in the real checkout.
set -euo pipefail

if [ "$#" -eq 0 ]; then
  echo "usage: ${0##*/} <command> [args...]" >&2
  exit 2
fi

repo=$(git rev-parse --show-toplevel)
rel=$(realpath --relative-to="$repo" "$PWD")
mnt="${TMPDIR:?TMPDIR must be set}/with-writable-tmp-checkout"
mkdir -p "$mnt"

# The inner script takes its arguments positionally, so the single quotes
# are deliberate — nothing is meant to expand in the outer shell.
# shellcheck disable=SC2016
exec unshare --map-root-user --mount sh -c '
  set -eu
  mount --bind "$1" "$2"
  mount -t tmpfs tmpfs /tmp
  cd "$2/$3"
  shift 3
  exec "$@"
' sh "$repo" "$mnt" "$rel" "$@"
