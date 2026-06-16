namespace Prql.Compiler;

/// <summary>
/// Location within a source file.
/// </summary>
/// <param name="StartLine">Start line.</param>
/// <param name="StartCol">Start column.</param>
/// <param name="EndLine">End line.</param>
/// <param name="EndCol">End column.</param>
public readonly record struct SourceLocation(
    ulong StartLine,
    ulong StartCol,
    ulong EndLine,
    ulong EndCol);
