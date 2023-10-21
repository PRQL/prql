using System.Runtime.InteropServices;

namespace Prql.Compiler
{
    /// <summary>
    /// Location within a source file.
    /// </summary>
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
    public struct SourceLocation
    {
        /// <summary>
        /// Start line.
        /// </summary>
        public int StartLine { get; set; }

        /// <summary>
        /// Start column.
        /// </summary>
        public int StartCol { get; set; }

        /// <summary>
        /// End line.
        /// </summary>
        public int EndLine { get; set; }

        /// <summary>
        /// End column.
        /// </summary>
        public int EndCol { get; set; }
    }
}
