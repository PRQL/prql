namespace Prql.Compiler;

/// <summary>
/// Identifier of a location in source.
/// Contains offsets in terms of chars.
/// </summary>
/// <param name="Start">Start offset.</param>
/// <param name="End">End offset.</param>
public readonly record struct Span(ulong Start, ulong End);
