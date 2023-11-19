using Prql.Compiler;

namespace Prql.Compiler.Tests;

public sealed class CompilerTest
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
