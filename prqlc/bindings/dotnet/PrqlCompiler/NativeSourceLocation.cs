using System.Runtime.InteropServices;

namespace Prql.Compiler;

[StructLayout(LayoutKind.Sequential)]
internal readonly struct NativeSourceLocation
{
    public readonly UIntPtr StartLine;
    public readonly UIntPtr StartCol;
    public readonly UIntPtr EndLine;
    public readonly UIntPtr EndCol;
}
