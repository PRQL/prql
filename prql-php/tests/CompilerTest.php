<?php declare(strict_types=1);

use Prql\Compiler\Compiler;
use Prql\Compiler\Options;
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
        $code = "int prql_to_pl(const char *prql_query, char *out);";
        $ffi = FFI::cdef($code, "src/libprql_lib.so");
        $this->assertInstanceOf(FFI::class, $ffi);
    }

    public function testInvalidQueryThrows(): void
    {
        $this->expectException(\InvalidArgumentException::class);

        $prql = new Compiler();
        $prql->compile("invalid");
    }

    public function testCompileWorks(): void
    {
        $options = new Options();
        $options->format = false;
        $options->signature_comment = false;
        $options->target = "sql.mssql";
        $prql = new Compiler();

        $expected = "SELECT * FROM employees";
        $actual = $prql->compile("from employees", $options);

        $this->assertEquals($expected, $actual);
    }

    public function testPrqlToPLWorks(): void
    {
        $prql = new Compiler();

        $pl = $prql->prqlToPL("from employees");

        $this->assertNotNull($pl);
    }
}
