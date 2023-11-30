# prql-dotnet

`prql-net` offers PRQL bindings for .NET bindings as a `netstandard2.0` library.

It provides the `PrqlCompiler` class which contains the `ToJson` and `ToSql`
static methods.

It's still at an early stage, and isn't published to NuGet. Contributions are
welcome.

## Installation

Make sure that `libprqlc_lib.so` (Linux), `libprqlc_lib.dylib` (macOS) or
`libprqlc_lib.dll` (Windows) is in the project's `bin` directory together with
`PrqlCompiler.dll` and the rest of the project's compiled files. I.e.
`{your_project}/bin/Debug/net7.0/`.

The `libprqlc_lib` library gets dynamically imported at runtime.

## Usage

```csharp
using Prql.Compiler;

var options = new PrqlCompilerOptions
{
    Format = false,
    SignatureComment = false,
};
var sql = PrqlCompiler.Compile("from employees", options);
Console.WriteLine(sql);
```

## TODO

This is currently at 0.1.0 because we're waiting to update prqlc-clib for the
latest API. When we've done that, we can match the version here with the broader
PRQL version.
