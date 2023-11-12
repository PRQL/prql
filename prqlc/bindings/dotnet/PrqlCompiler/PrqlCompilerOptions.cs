using System.Runtime.InteropServices;

namespace Prql.Compiler
{
    /// <summary>
    /// Compilation options for SQL backend of the compiler.
    /// </summary>
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
    public record struct PrqlCompilerOptions
    (
        /// <summary>
        /// Pass generated SQL string trough a formatter that splits it into
        /// multiple lines and prettifies indentation and spacing.
        /// </summary>
        /// <remarks>Defaults to <c>true</c>.</remarks>
        bool Format = true,

        /// <summary>
        /// Target and dialect to compile to.
        /// </summary>
        string Target = "",

        /// <summary>
        /// Emits the compiler signature as a comment after generated SQL.
        /// </summary>
        /// <remarks>Defaults to <c>true</c>.</remarks>
        bool SignatureComment = true
    )
    { }
}
