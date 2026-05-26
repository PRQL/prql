# PRQL C library

## Description

This module compiles PRQL as a library (both `.a` and `.so` are generated). This
allows embedding in languages that support FFI — for example, Golang.

## Linking

See [examples/minimal-c/Makefile](examples/minimal-c/Makefile) for the canonical
link flags used to build the bundled C example.

To link against the static library from another build system, point the linker
at the directory containing `libprqlc_c.a` and add `prqlc_c` plus its system
dependencies, for example:

`CGO_LDFLAGS="-L/path/to/target/release -lprqlc_c -pthread -ldl -lm" go build`

(On macOS, also add `-framework CoreFoundation`.)

## Examples

- [examples/minimal-c/main.c](examples/minimal-c/main.c) — minimal C example
  covering `compile`, custom `Options`, error handling, and the staged
  `prql_to_pl` / `pl_to_rq` entry points.
- [examples/minimal-cpp](examples/minimal-cpp) — the same flow using the
  generated C++ header.
- [examples/minimal-zig](examples/minimal-zig) — a Zig example using `@cImport`
  against `prqlc.h`.

The full FFI surface is documented inline in [prqlc.h](prqlc.h).

## Development

### Headers

The C & C++ header files `prqlc.h` & `prqlc.hpp` were generated using
[cbindgen](https://github.com/eqrion/cbindgen). To generate a new one run:

```sh
task build-prqlc-c-header
```

...or copy & paste the commands from the Taskfile.
