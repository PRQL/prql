using System.Runtime.InteropServices;

namespace Prql.Compiler;

/// <summary>
/// The PRQL compiler transpiles PRQL queries.
/// </summary>
public static partial class PrqlCompiler
{
    private const string LibraryName = "libprqlc_c";

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

        return Compile(prqlQuery, new PrqlCompilerOptions());
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

        ArgumentNullException.ThrowIfNull(options);

        var targetPtr = options.Target is null
            ? IntPtr.Zero
            : Marshal.StringToCoTaskMemUTF8(options.Target);
        try
        {
            var nativeOptions = new NativePrqlCompilerOptions(options, targetPtr);
            var nativeResult = CompileExtern(prqlQuery, ref nativeOptions);
            return new Result(nativeResult);
        }
        finally
        {
            if (targetPtr != IntPtr.Zero)
            {
                Marshal.FreeCoTaskMem(targetPtr);
            }
        }
    }

    /// <summary>
    /// Build PL AST from a PRQL string.
    /// </summary>
    /// <param name="prqlQuery">A PRQL query.</param>
    /// <returns>JSON.</returns>
    /// <exception cref="ArgumentException"><paramref name="prqlQuery"/> is null or empty.</exception>
    /// <exception cref="FormatException"><paramref name="prqlQuery"/> cannot be compiled.</exception>
    /// <remarks>https://docs.rs/prqlc/latest/</remarks>
    public static Result PrqlToPl(string prqlQuery)
    {
        if (string.IsNullOrEmpty(prqlQuery))
        {
            throw new ArgumentException(nameof(prqlQuery));
        }

        var nativeResult = PrqlToPlExtern(prqlQuery);
        return new Result(nativeResult);
    }

    /// <summary>
    /// Finds variable references, validates functions calls, determines frames and converts PL to RQ.
    /// </summary>
    /// <param name="plJson">A PRQL query.</param>
    /// <returns>JSON.</returns>
    /// <exception cref="ArgumentException"><paramref name="plJson"/> is null or empty.</exception>
    /// <exception cref="FormatException"><paramref name="plJson"/> cannot be compiled.</exception>
    /// <remarks>https://docs.rs/prqlc/latest/</remarks>
    public static Result PlToRq(string plJson)
    {
        if (string.IsNullOrEmpty(plJson))
        {
            throw new ArgumentException(nameof(plJson));
        }

        var nativeResult = PlToRqExtern(plJson);
        return new Result(nativeResult);
    }

    /// <summary>
    /// Convert RQ AST into an SQL string.
    /// </summary>
    /// <param name="rqJson">RQ string in JSON format.</param>
    /// <param name="options">PRQL compiler options.</param>
    /// <returns>JSON.</returns>
    /// <exception cref="ArgumentException"><paramref name="rqJson"/> is null or empty.</exception>
    /// <exception cref="ArgumentNullException"><paramref name="options"/> is <c>null</c>.</exception>
    /// <exception cref="FormatException"><paramref name="rqJson"/> cannot be compiled.</exception>
    /// <remarks>https://docs.rs/prqlc/latest/</remarks>
    public static Result RqToSql(string rqJson, PrqlCompilerOptions options)
    {
        if (string.IsNullOrEmpty(rqJson))
        {
            throw new ArgumentException(nameof(rqJson));
        }

        ArgumentNullException.ThrowIfNull(options);

        var targetPtr = options.Target is null
            ? IntPtr.Zero
            : Marshal.StringToCoTaskMemUTF8(options.Target);
        try
        {
            var nativeOptions = new NativePrqlCompilerOptions(options, targetPtr);
            var nativeResult = RqToSqlExtern(rqJson, ref nativeOptions);
            return new Result(nativeResult);
        }
        finally
        {
            if (targetPtr != IntPtr.Zero)
            {
                Marshal.FreeCoTaskMem(targetPtr);
            }
        }
    }

    [LibraryImport(LibraryName, EntryPoint = "compile", StringMarshalling = StringMarshalling.Utf8)]
    private static partial NativeResult CompileExtern(string prqlQuery, ref NativePrqlCompilerOptions options);

    [LibraryImport(LibraryName, EntryPoint = "prql_to_pl", StringMarshalling = StringMarshalling.Utf8)]
    private static partial NativeResult PrqlToPlExtern(string prqlQuery);

    [LibraryImport(LibraryName, EntryPoint = "pl_to_rq", StringMarshalling = StringMarshalling.Utf8)]
    private static partial NativeResult PlToRqExtern(string plJson);

    [LibraryImport(LibraryName, EntryPoint = "rq_to_sql", StringMarshalling = StringMarshalling.Utf8)]
    private static partial NativeResult RqToSqlExtern(string rqJson, ref NativePrqlCompilerOptions options);
}
