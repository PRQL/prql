using System.Runtime.InteropServices;

namespace Prql.Compiler;

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSourceLocation
{
    public UIntPtr StartLine;
    public UIntPtr StartCol;
    public UIntPtr EndLine;
    public UIntPtr EndCol;
}
