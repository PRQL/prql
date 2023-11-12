# prql-dotnet

`PrqlCompiler` offers PRQL bindings for .NET bindings as `net6.0` and `net7.0`
libraries.

It provides the `PrqlCompiler` class which contains the `Compile`, `PrqlToPl`,
`PlToRq` and `RqToSql` static methods.

It's still at an early stage, and isn't published to NuGet. Contributions are
welcome.

## Installation

Current project and package only handles Windows native library `prqlc.dll`.
Handling of `prqlc.so` (Linux), `prqlc.dylib` (macOS) is work in progress.

For consumer of this package, ensure that `prqlc.dll` is in your project's `bin`
(i.e. `{your_project}/bin/Debug/net7.0/`) directory together with
`PrqlCompiler.dll` and the rest of your project's compiled files.

If it's not the case, ensure that you specified a runtime parameter when
publishing your project. I.e.
`dotnet publish YourProject.csproj --runtime win-x64 --framework net6.0 -o Publish\net6.0-win-x64`
.

If you're using the package `PrqlCompiler` in a test project, identically, don't
forget to specify the runtime. I.e.
`dotnet test YourProject.Tests.csproj --runtime win-x64 --framework net6.0` .

The list of currently supported runtimes is:

-   win-x64

The `prqlc` library gets dynamically imported at runtime ans id not needed at
compiled time

## Usage

```csharp
using Prql.Compiler;

var options = new PrqlCompilerOptions
{
    Format = false,
    SignatureComment = false,
    Target = "sql.mysql"
};
var sql = PrqlCompiler.Compile("from employees", options);
Console.WriteLine(sql);
```

# TODO

We're waiting to include the build and tests of this package into the
GitHub-actions. When we've done that, we can match the version here with the
broader PRQL version.
