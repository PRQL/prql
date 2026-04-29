using System.Runtime.InteropServices;

namespace Prql.Compiler;

[StructLayout(LayoutKind.Sequential)]
internal readonly struct NativeSpan
{
    public readonly UIntPtr Start;
    public readonly UIntPtr End;
}
