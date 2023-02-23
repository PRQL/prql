using System.Runtime.InteropServices;

namespace Prql.Compiler
{
    /// <summary>
    /// Compilation options for SQL backend of the compiler.
    /// </summary>
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
    public class PrqlCompilerOptions
    {
        /// <summary>
        /// Pass generated SQL string trough a formatter that splits it into
        /// multiple lines and prettifies indentation and spacing.
        /// </summary>
        /// <remarks>Defaults to <c>true</c>.</remarks>
        public bool Format { get; set; } = true;

        /// <summary>
        /// Target and dialect to compile to.
        /// </summary>
        public string Target { get; set; }

        /// <summary>
        /// Emits the compiler signature as a comment after generated SQL.
        /// </summary>
        /// <remarks>Defaults to <c>true</c>.</remarks>
        public bool SignatureComment { get; set; } = true;
    }
}
