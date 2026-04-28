using System.Runtime.InteropServices;

namespace Prql.Compiler;

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSpan
{
    public UIntPtr Start;
    public UIntPtr End;
}
