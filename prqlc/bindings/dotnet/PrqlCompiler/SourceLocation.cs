namespace Prql.Compiler
{
    /// <summary>
    /// Location within a source file.
    /// </summary>
    public struct SourceLocation
    {
        /// <summary>
        /// Start line.
        /// </summary>
        public ulong StartLine { get; set; }

        /// <summary>
        /// Start column.
        /// </summary>
        public ulong StartCol { get; set; }

        /// <summary>
        /// End line.
        /// </summary>
        public ulong EndLine { get; set; }

        /// <summary>
        /// End column.
        /// </summary>
        public ulong EndCol { get; set; }
    }
}
