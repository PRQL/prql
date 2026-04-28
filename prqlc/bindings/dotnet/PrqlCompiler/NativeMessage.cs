using System.Runtime.InteropServices;

namespace Prql.Compiler;

[StructLayout(LayoutKind.Sequential)]
internal struct NativeMessage
{
    public MessageKind Kind;
    public IntPtr Code;
    public IntPtr Reason;
    public IntPtr Hint;
    public IntPtr Span;
    public IntPtr Display;
    public IntPtr Location;
}
