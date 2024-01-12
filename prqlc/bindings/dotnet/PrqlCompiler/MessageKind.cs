using System;

namespace Prql.Compiler;

/// <summary>
/// Compile message kind. Currently only Error is implemented.
/// </summary>
[Serializable]
public enum MessageKind
{
    /// <summary>
    /// Error message.
    /// </summary>
    Error,
    /// <summary>
    /// Warning message.
    /// </summary>
    Warning,
    /// <summary>
    /// Lint message.
    /// </summary>
    Lint
}
