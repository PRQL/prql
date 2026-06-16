using System.Runtime.InteropServices;

namespace Prql.Compiler;

[StructLayout(LayoutKind.Sequential)]
internal readonly struct NativeResult
{
#pragma warning disable CS0649 // Field is never assigned to
    public readonly IntPtr Output;
    public readonly IntPtr Messages;
    public readonly UIntPtr MessagesLen;
#pragma warning restore CS0649 // Field is never assigned to
}
