using System.Runtime.InteropServices;

namespace Prql.Compiler;

/// <summary>
/// Identifier of a location in source.
/// Contains offsets in terms of chars.
/// </summary>
[StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
public struct Span
{
    /// <summary>
    /// Start offset.
    /// </summary>
    public int Start { get; set; }

    /// <summary>
    /// End offset.
    /// </summary>
    public int End { get; set; }
}
