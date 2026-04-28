using System.Runtime.InteropServices;

namespace Prql.Compiler;

[StructLayout(LayoutKind.Sequential)]
internal struct NativePrqlCompilerOptions
{
    public byte Format;
    public IntPtr Target;
    public byte SignatureComment;

    public NativePrqlCompilerOptions(PrqlCompilerOptions options, IntPtr targetPtr)
    {
        Format = options.Format ? (byte)1 : (byte)0;
        Target = targetPtr;
        SignatureComment = options.SignatureComment ? (byte)1 : (byte)0;
    }
}
