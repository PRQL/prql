using System.Runtime.InteropServices;
using System.Text;

namespace Prql.Compiler;

/// <summary>
/// Result of compilation.
/// </summary>
public sealed partial class Result
{
    private const string LibraryName = "libprqlc_c";

    private readonly IReadOnlyCollection<Message> _messages;

    internal Result(NativeResult result)
    {
        try
        {
            Output = PtrToUtf8String(result.Output) ?? string.Empty;

            var len = checked((int)result.MessagesLen.ToUInt64());
            var messages = new List<Message>(len);
            var nativeMessageSize = Marshal.SizeOf<NativeMessage>();

            for (var i = 0; i < len; i++)
            {
                var entryPtr = IntPtr.Add(result.Messages, i * nativeMessageSize);
                var native = Marshal.PtrToStructure<NativeMessage>(entryPtr);
                messages.Add(ConvertMessage(native));
            }

            _messages = messages.AsReadOnly();
        }
        finally
        {
            ResultDestroyExtern(result);
        }
    }

    /// <summary>
    /// The compiler output.
    /// </summary>
    public string Output { get; }

    /// <summary>
    /// Error, warning and lint messages.
    /// </summary>
    public IReadOnlyCollection<Message> Messages => _messages;

    private static Message ConvertMessage(NativeMessage native)
    {
        return new Message
        {
            Kind = native.Kind,
            Code = PtrToUtf8StringIndirect(native.Code),
            Reason = PtrToUtf8String(native.Reason) ?? string.Empty,
            Hint = PtrToUtf8StringIndirect(native.Hint),
            Span = ReadStruct<NativeSpan>(native.Span) is NativeSpan s
                ? new Span(s.Start.ToUInt64(), s.End.ToUInt64())
                : null,
            Display = PtrToUtf8StringIndirect(native.Display),
            Location = ReadStruct<NativeSourceLocation>(native.Location) is NativeSourceLocation l
                ? new SourceLocation(
                    l.StartLine.ToUInt64(),
                    l.StartCol.ToUInt64(),
                    l.EndLine.ToUInt64(),
                    l.EndCol.ToUInt64())
                : null,
        };
    }

    private static T? ReadStruct<T>(IntPtr ptr) where T : struct
    {
        if (ptr == IntPtr.Zero)
        {
            return null;
        }
        return Marshal.PtrToStructure<T>(ptr);
    }

    private static string? PtrToUtf8StringIndirect(IntPtr pointerToPointer)
    {
        if (pointerToPointer == IntPtr.Zero)
        {
            return null;
        }
        var stringPtr = Marshal.ReadIntPtr(pointerToPointer);
        return PtrToUtf8String(stringPtr);
    }

    private static string? PtrToUtf8String(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero)
        {
            return null;
        }
        var len = 0;
        while (Marshal.ReadByte(ptr, len) != 0)
        {
            len++;
        }
        if (len == 0)
        {
            return string.Empty;
        }
        var bytes = new byte[len];
        Marshal.Copy(ptr, bytes, 0, len);
        return Encoding.UTF8.GetString(bytes);
    }

    [LibraryImport(LibraryName, EntryPoint = "result_destroy")]
    private static partial void ResultDestroyExtern(NativeResult res);
}
