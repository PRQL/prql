namespace Prql.Compiler
{
    /// <summary>
    /// Identifier of a location in source.
    /// Contains offsets in terms of chars.
    /// </summary>
    public struct Span
    {
        /// <summary>
        /// Start offset.
        /// </summary>
        public ulong Start { get; set; }

        /// <summary>
        /// End offset.
        /// </summary>
        public ulong End { get; set; }
    }
}
