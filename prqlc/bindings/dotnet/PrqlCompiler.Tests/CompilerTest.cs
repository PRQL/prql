using Prql.Compiler;

namespace Prql.Compiler.Tests;

sealed public class CompilerTest
{
    [Fact]
    public void ToCompile_Works()
    {
    	// Arrange
        var expected = "SELECT * FROM employees";
        var options = new PrqlCompilerOptions
        {
            Format = false,
            SignatureComment = false,
            Target = "sql.mssql"
        };

        // Act
        var result = PrqlCompiler.Compile("from employees", options);

        // Assert
        Assert.Equal(expected, result.Output);
    }

    [Fact]
    public void Compile_ReportsErrorMessages()
    {
        // Arrange — `unknown_function` is not defined, producing an error
        // message whose optional fields (Span, Display, Location) get
        // populated. This validates pointer dereferencing for indirect
        // string fields and Span/Location pointers in the FFI struct layout.
        var query = "from employees | unknown_function col";

        // Act
        var result = PrqlCompiler.Compile(query);

        // Assert
        Assert.NotEmpty(result.Messages);
        var message = result.Messages.First();
        Assert.Equal(MessageKind.Error, message.Kind);
        Assert.False(string.IsNullOrEmpty(message.Reason));
        Assert.NotNull(message.Span);
        Assert.NotNull(message.Location);
        Assert.False(string.IsNullOrEmpty(message.Display));
    }

    [Fact]
    public void Compile_ThrowsArgumentNullException_WhenOptionsNull()
    {
        Assert.Throws<ArgumentNullException>(() => PrqlCompiler.Compile("from x", null!));
    }

    [Fact]
    public void RqToSql_ThrowsArgumentNullException_WhenOptionsNull()
    {
        Assert.Throws<ArgumentNullException>(() => PrqlCompiler.RqToSql("{}", null!));
    }

    [Fact]
    public void TestOtherFunctions()
    {
        // Arrange
        var query = """
            let a = (from employees | take 10)

            from a | select {first_name}
            """;
        var options = new PrqlCompilerOptions();

        // Act and assert
        var pl = PrqlCompiler.PrqlToPl(query);
        Assert.Empty(pl.Messages);

        var rq = PrqlCompiler.PlToRq(pl.Output);
        Assert.Empty(rq.Messages);

        var via_json = PrqlCompiler.RqToSql(rq.Output, options);
        Assert.Empty(via_json.Messages);

        var direct = PrqlCompiler.Compile(query);
        Assert.Empty(direct.Messages);

        Assert.Equal(via_json.Messages, direct.Messages);
        Assert.Equal(via_json.Output, direct.Output);
    }
}
