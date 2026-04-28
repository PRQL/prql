using System.Runtime.InteropServices;

namespace Prql.Compiler;

[StructLayout(LayoutKind.Sequential)]
internal struct NativeResult
{
#pragma warning disable CS0649 // Field is never assigned to
    public IntPtr Output;
    public IntPtr Messages;
    public UIntPtr MessagesLen;
#pragma warning restore CS0649 // Field is never assigned to
}
