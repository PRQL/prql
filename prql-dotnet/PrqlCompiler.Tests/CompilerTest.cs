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
}
