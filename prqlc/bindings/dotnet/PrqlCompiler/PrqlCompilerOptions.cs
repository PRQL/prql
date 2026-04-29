namespace Prql.Compiler;

/// <summary>
/// Compilation options for SQL backend of the compiler.
/// </summary>
public sealed record PrqlCompilerOptions
{
    /// <summary>
    /// Pass generated SQL string through a formatter that splits it into
    /// multiple lines and prettifies indentation and spacing.
    /// </summary>
    /// <remarks>Defaults to <c>true</c>.</remarks>
    public bool Format { get; init; } = true;

    /// <summary>
    /// Target and dialect to compile to.
    /// </summary>
    public string? Target { get; init; }

    /// <summary>
    /// Emits the compiler signature as a comment after generated SQL.
    /// </summary>
    /// <remarks>Defaults to <c>true</c>.</remarks>
    public bool SignatureComment { get; init; } = true;
}
