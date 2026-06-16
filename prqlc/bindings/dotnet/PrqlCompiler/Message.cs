namespace Prql.Compiler;

/// <summary>
/// Compile result message.
/// </summary>
public sealed record Message
{
    /// <summary>
    /// Message kind. Currently only Error is implemented.
    /// </summary>
    public MessageKind Kind { get; init; }

    /// <summary>
    /// Machine-readable identifier of the error. May be null.
    /// </summary>
    public string? Code { get; init; }

    /// <summary>
    /// Plain text of the error.
    /// </summary>
    public string Reason { get; init; } = string.Empty;

    /// <summary>
    /// A suggestion of how to fix the error. May be null.
    /// </summary>
    public string? Hint { get; init; }

    /// <summary>
    /// Character offset of error origin within a source file. May be null.
    /// </summary>
    public Span? Span { get; init; }

    /// <summary>
    /// Annotated code, containing cause and hints. May be null.
    /// </summary>
    public string? Display { get; init; }

    /// <summary>
    /// Line and column number of error origin within a source file. May be null.
    /// </summary>
    public SourceLocation? Location { get; init; }
}
