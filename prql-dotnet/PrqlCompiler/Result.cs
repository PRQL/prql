using System.Collections.Generic;
using System.Linq;

namespace Prql.Compiler
{
    /// <summary>
    /// Result of compilation.
    /// </summary>
    public class Result
    {
        private readonly IReadOnlyCollection<Message> _messages;

        internal Result(string output, IEnumerable<Message> messages)
        {
            Output = output;
            _messages = messages.ToList().AsReadOnly();
        }

        /// <summary>
        /// @var string
        /// </summary>
        public string Output { get; }

        /// <summary>
        /// @var array<Message>
        /// </summary>
        public IReadOnlyCollection<Message> Messages => _messages;
    }
}