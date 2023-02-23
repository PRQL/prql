using Prql.Compiler;

namespace Prql.Compiler.Tests;

sealed public class CompilerTest
{
    [Fact]
    public void ToCompile_Works()
    {
    	// Arrange
        var expected = "SELECT * FROM employees";

        // Act
        var options = new PrqlCompilerOptions
        {
            Format = false,
            SignatureComment = false,
        };
        var sqlQuery = PrqlCompiler.Compile("from employees", options);

        // Assert
        Assert.Equal(expected, sqlQuery);
    }
}
