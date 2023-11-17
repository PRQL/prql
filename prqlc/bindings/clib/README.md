# PRQL C library

## Description

This module compiles PRQL as a library (both `.a` and `.so` are generated). This
allows embedding in languages that support FFI - looking at Golang.

## Linking

See [examples/minimal-c/Makefile](examples/minimal-c/Makefile).

Copy the `.a` and `.so` files in a convenient place and add the following
compile flags to Go (cgo):

`CGO_LDFLAGS="-L/path/to/libprqlc_lib.a -lprqlc -pthread -ldl" go build`

## Examples

For a minimal example, see
[examples/minimal-c/main.c](examples/minimal-c/main.c).

Below is an example from an actual application that is using PRQL in Go.

```go
package prql

/*


#include <stdlib.h>

int to_sql(char *prql_query, char *sql_query);
int to_json(char *prql_query, char *json_query);

*/
import "C"

import (
    "errors"
    "strings"
    "unsafe"
)

// ToSQL converts a PRQL query to SQL
func ToSQL(prql string) (string, error) {
    // buffer length should not be less than 1K because we may get an error
    // from the PRQL compiler with a very short query
    cStringBufferLength := 1024

    // allocate a buffer 3 times the length of the PRQL query to store the
    // generated SQL query
    if len(prql)*3 > cStringBufferLength {
        cStringBufferLength = len(prql) * 3
    }

    // preallocate the buffer
    cstr := C.CString(strings.Repeat(" ", cStringBufferLength))
    defer C.free(unsafe.Pointer(cstr))

    // convert the PRQL query to SQL
    res := C.to_sql(C.CString(prql), cstr)
    if res == 0 {
        return C.GoString(cstr), nil
    }

    return "", errors.New(C.GoString(cstr))
}

// ToJSON converts a PRQL query to JSON
func ToJSON(prql string) (string, error) {
    // buffer length should not be less than 1K because we may get an error
    cStringBufferLength := 1024
    if len(prql)*3 > cStringBufferLength {
        cStringBufferLength = len(prql) * 10
    }

    // preallocate the buffer
    cstr := C.CString(strings.Repeat(" ", cStringBufferLength))
    defer C.free(unsafe.Pointer(cstr))

    // convert the PRQL query to SQL
    res := C.to_json(C.CString(prql), cstr)
    if res == 0 {
        return C.GoString(cstr), nil
    }

    return "", errors.New(C.GoString(cstr))
}
```

## Development

### C header file

The C header file `libprqlc.h` was generated using
[cbindgen](https://github.com/eqrion/cbindgen). To generate a new one run:

    cargo install --force cbindgen
    cbindgen --crate prqlc-clib --output libprqlc.h

### C++ header file

The C++ header file `libprqlc.hpp` was generated using cbindgen. To generate a
new one run:

    cbindgen --crate prqlc-clib --lang C++ --output libprqlc.hpp
