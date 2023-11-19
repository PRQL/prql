using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;

namespace Prql.Compiler;

/// <summary>
/// Result of compilation.
/// </summary>
public class Result
{
    private readonly IReadOnlyCollection<Message> _messages;

    internal Result(NativeResult result)
    {
        Output = result.Output;

        var messages = new List<Message>();

        for (var i = 0; i < result.MessagesLen; i++)
        {
            messages.Add(Marshal.PtrToStructure<Message>(result.Messages));
        }

        _messages = messages.ToList().AsReadOnly();
    }

    /// <summary>
    /// The compiler output.
    /// </summary>
    public string Output { get; }

    /// <summary>
    /// Error, warning and lint messages.
    /// </summary>
    public IReadOnlyCollection<Message> Messages => _messages;
}
