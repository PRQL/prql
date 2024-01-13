using System;
using System.Runtime.InteropServices;

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
        public static Result Compile(string prqlQuery)
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
        public static Result Compile(string prqlQuery, PrqlCompilerOptions options)
        {
            if (string.IsNullOrEmpty(prqlQuery))
            {
                throw new ArgumentException(nameof(prqlQuery));
            }

            if (options is null)
            {
                throw new ArgumentException(nameof(options));
            }

            var nativeOptions = new NativePrqlCompilerOptions(options);
            var nativeResult = CompileExtern(prqlQuery, ref nativeOptions);
            var result = new Result(nativeResult);

            return result;
        }

        /// <summary>
        /// Build PL AST from a PRQL string.
        /// </summary>
        /// <param name="prqlQuery">A PRQL query.</param>
        /// <returns>JSON.</returns>
        /// <exception cref="ArgumentException"><paramref name="prqlQuery"/> is null or empty.</exception>
        /// <exception cref="FormatException"><paramref name="prqlQuery"/> cannot be compiled.</exception>
        /// <remarks>https://docs.rs/prql-compiler/latest/prql_compiler/ir/pl</remarks>
        public static Result PrqlToPl(string prqlQuery)
        {
            if (string.IsNullOrEmpty(prqlQuery))
            {
                throw new ArgumentException(nameof(prqlQuery));
            }

            var nativeResult = PrqlToPlExtern(prqlQuery);
            var result = new Result(nativeResult);

            return result;
        }

        /// <summary>
        /// Finds variable references, validates functions calls, determines frames and converts PL to RQ.
        /// </summary>
        /// <param name="plJson">A PRQL query.</param>
        /// <returns>JSON.</returns>
        /// <exception cref="ArgumentException"><paramref name="plJson"/> is null or empty.</exception>
        /// <exception cref="FormatException"><paramref name="plJson"/> cannot be compiled.</exception>
        /// <remarks>https://docs.rs/prql-compiler/latest/prql_compiler/ast</remarks>
        public static Result PlToRq(string plJson)
        {
            if (string.IsNullOrEmpty(plJson))
            {
                throw new ArgumentException(nameof(plJson));
            }

            var nativeResult = PlToRqExtern(plJson);
            var result = new Result(nativeResult);

            return result;
        }

        /// <summary>
        /// Convert RQ AST into an SQL string.
        /// </summary>
        /// <param name="rqJson">RQ string in JSON format.</param>
        /// <param name="options">PRQL compiler options.</param>
        /// <returns>JSON.</returns>
        /// <exception cref="ArgumentException"><paramref name="prqlQuery"/> is null or empty.</exception>
        /// <exception cref="ArgumentNullException"><paramref name="options"/> is <c>null</c>.</exception>
        /// <exception cref="FormatException"><paramref name="prqlQuery"/> cannot be compiled.</exception>
        /// <remarks>https://docs.rs/prql-compiler/latest/prql_compiler/ir/rq</remarks>
        public static Result RqToSql(string rqJson, PrqlCompilerOptions options)
        {
            if (string.IsNullOrEmpty(rqJson))
            {
                throw new ArgumentException(nameof(rqJson));
            }

            if (options is null)
            {
                throw new ArgumentException(nameof(options));
            }

            var nativeOptions = new NativePrqlCompilerOptions(options);
            var nativeResult = RqToSqlExtern(rqJson, ref nativeOptions);
            var result = new Result(nativeResult);

            return result;
        }

        [DllImport("libprqlc_c", EntryPoint = "compile")]
        private static extern NativeResult CompileExtern(string prqlQuery, ref NativePrqlCompilerOptions options);

        [DllImport("libprqlc_c", EntryPoint = "prql_to_pl")]
        private static extern NativeResult PrqlToPlExtern(string prqlQuery);

        [DllImport("libprqlc_c", EntryPoint = "pl_to_rq")]
        private static extern NativeResult PlToRqExtern(string plJson);

        [DllImport("libprqlc_c", EntryPoint = "rq_to_sql")]
        private static extern NativeResult RqToSqlExtern(string rqJson, ref NativePrqlCompilerOptions options);
    }
}
