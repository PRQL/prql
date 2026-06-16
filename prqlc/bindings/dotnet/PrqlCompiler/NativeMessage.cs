using System.Runtime.InteropServices;

namespace Prql.Compiler;

[StructLayout(LayoutKind.Sequential)]
internal readonly struct NativeMessage
{
    public readonly MessageKind Kind;
    public readonly IntPtr Code;
    public readonly IntPtr Reason;
    public readonly IntPtr Hint;
    public readonly IntPtr Span;
    public readonly IntPtr Display;
    public readonly IntPtr Location;
}
