using System;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;

namespace Prql.Compiler
{
    /// <summary>
    /// The PRQL compiler transpiles RPQL queries.
    /// </summary>
    public static class PrqlCompiler
    {
        /// <summary>
        /// Compile a PRQL string into a SQL string.
        /// </summary>
        /// <param name="prqlQuery">A PRQL query.</param>
        /// <returns>SQL query.</returns>
        /// <exception cref="ArgumentException"><paramref name="prqlQuery"/> is null or empty.</exception>
        /// <exception cref="FormatException"><paramref name="prqlQuery"/> cannot be compiled.</exception>
        public static string Compile(string prqlQuery)
        {
            if (string.IsNullOrEmpty(prqlQuery))
            {
                throw new ArgumentException(nameof(prqlQuery));
            }

            var options = new PrqlCompilerOptions();

            return Compile(prqlQuery, options);
        }

        /// <summary>
        /// Compile a PRQL string into a SQL string.
        /// </summary>
        /// <param name="prqlQuery">A PRQL query.</param>
        /// <param name="options">PRQL compiler options.</param>
        /// <returns>SQL query.</returns>
        /// <exception cref="ArgumentException"><paramref name="prqlQuery"/> is null or empty.</exception>
        /// <exception cref="ArgumentNullException"><paramref name="options"/> is <c>null</c>.</exception>
        /// <exception cref="FormatException"><paramref name="prqlQuery"/> cannot be compiled.</exception>
        public static string Compile(string prqlQuery, PrqlCompilerOptions options)
        {
            if (string.IsNullOrEmpty(prqlQuery))
            {
                throw new ArgumentException(nameof(prqlQuery));
            }

            byte[] bytes = new byte[1024];
            if (CompileExtern(prqlQuery, ref options, bytes) != 0)
            {
                throw new FormatException("Could not compile query.");
            }

            bytes = bytes.TakeWhile(x => ((char)x) != '\0').ToArray();

            return Encoding.UTF8.GetString(bytes);
        }

        /// <summary>
        /// Compile a PRQL string into a JSON string.
        /// </summary>
        /// <param name="prqlQuery">A PRQL query.</param>
        /// <returns>JSON.</returns>
        /// <exception cref="ArgumentException"><paramref name="prqlQuery"/> is null or empty.</exception>
        /// <exception cref="FormatException"><paramref name="prqlQuery"/> cannot be compiled.</exception>
        public static string ToJson(string prqlQuery)
        {
            if (string.IsNullOrEmpty(prqlQuery))
            {
                throw new ArgumentException(nameof(prqlQuery));
            }

            byte[] bytes = new byte[1024];
            if (ToJsonExtern(prqlQuery, bytes) != 0)
            {
                throw new FormatException("Could not compile query.");
            }

            bytes = bytes.TakeWhile(x => ((char)x) != '\0').ToArray();

            return Encoding.UTF8.GetString(bytes);
        }

        [DllImport("libprql_lib", EntryPoint = "compile")]
        private static extern int CompileExtern(string prql_query, ref PrqlCompilerOptions options, byte[] sql_query);

        [DllImport("libprql_lib", EntryPoint = "to_json")]
        private static extern int ToJsonExtern(string prql_query, byte[] json);
    }
}
