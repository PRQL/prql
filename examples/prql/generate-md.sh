#!/bin/bash

for in in examples/prql/*.prql; do
  echo $in:
  out=examples/$(basename -s .prql $in)
  
  if prql compile $in -o $out.sql; then
    (
      echo '```elm';
      cat $in
      echo -e '```\n\n```elm'
      cat $out.sql
      echo '```'
    ) > $out.md
    echo 'done'
  fi
  
  rm $out.sql
done