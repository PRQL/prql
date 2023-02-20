<?php declare(strict_types=1);

use Prql\Compiler\Compiler;
use PHPUnit\Framework\TestCase;

final class CompilerTest extends TestCase
{
    public function testFfiExtensionIsLoaded(): void
    {
        $this->assertTrue(extension_loaded("ffi"));
    }

    public function testPrqlLibraryFileExists(): void
    {
        $this->assertFileExists("src/libprql_lib.so");
    }

    public function testPrqlLibraryLoads(): void
    {
        $code = "int to_sql(char *prql_query, char *sql_query);";
        $ffi = FFI::cdef($code, "src/libprql_lib.so");
        $this->assertInstanceOf(FFI::class, $ffi);
    }

    public function testInvalidQueryThrows(): void
    {
        $this->expectException(\InvalidArgumentException::class);

        $prql = new Compiler();
        $prql->toSql("invalid");
    }

    public function testToSqlWorks(): void
    {
        $expected = <<<'EOD'
SELECT
  *
FROM
  employees

-- Generated by PRQL compiler version:0.5.0 (https://prql-lang.org)
EOD;
        $expected = substr($expected, 0, strpos($expected, "Generated by PRQL compiler"));
        $prql = new Compiler();
        $actual = $prql->toSql("from employees");
        $actual = substr($actual, 0, strpos($actual, "Generated by PRQL compiler"));
        $this->assertEquals($expected, $actual);
    }
}
