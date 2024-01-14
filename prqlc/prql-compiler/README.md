# PRQL compiler

`prqlc` is the reference implementation of a compiler from PRQL to SQL, written
in Rust.

Since the previous name of this crate was `prql-compiler`, we maintain a crate
with this name which re-exports `prqlc`'s items, allowing
backward-compatibility.

But we recommend you instead use [`prqlc`](../prqlc/README.md).
