using System.Runtime.InteropServices;

namespace Prql.Compiler;

/// <summary>
/// Compile result message.
/// </summary>
[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
public struct Message
{
    /// <summary>
    /// Message kind. Currently only Error is implemented.
    /// </summary>
    public MessageKind Kind { get; set; }

    /// <summary>
    /// Machine-readable identifier of the error.
    /// </summary>
    public string Code { get; set; }

    /// <summary>
    /// Plain text of the error.
    /// </summary>
    public string Reason { get; set; }

    /// <summary>
    /// A list of suggestions of how to fix the error.
    /// </summary>
    public string Hint { get; set; }

    /// <summary>
    /// Character offset of error origin within a source file.
    /// </summary>
    public Span Span { get; set; }

    /// <summary>
    /// Annotated code, containing cause and hints.
    /// </summary>
    public string Display { get; set; }

    /// <summary>
    /// Line and column number of error origin within a source file.
    /// </summary>
    public SourceLocation Location { get; set; }
}
