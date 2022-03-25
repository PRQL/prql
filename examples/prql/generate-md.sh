#!/bin/bash

set -euxo pipefail

for in in examples/prql/*.prql; do
  echo $in:
  out=examples/$(basename -s .prql $in)

  # Set the binary to the recently compiled version; if this moves to rust we
  # can avoid this.
  if ./target/debug/prql compile $in -o $out.sql; then
    (
      echo '```elm';
      cat $in
      echo -e '```\n\n```sql'
      cat $out.sql
      echo -e '\n```'
    ) > $out.md
    echo 'done'
  fi

  rm $out.sql
done
