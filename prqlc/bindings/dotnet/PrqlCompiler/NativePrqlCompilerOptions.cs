using System.Runtime.InteropServices;

namespace Prql.Compiler;

[StructLayout(LayoutKind.Sequential)]
internal readonly struct NativePrqlCompilerOptions
{
    public readonly byte Format;
    public readonly IntPtr Target;
    public readonly byte SignatureComment;

    public NativePrqlCompilerOptions(PrqlCompilerOptions options, IntPtr targetPtr)
    {
        Format = options.Format ? (byte)1 : (byte)0;
        Target = targetPtr;
        SignatureComment = options.SignatureComment ? (byte)1 : (byte)0;
    }
}
