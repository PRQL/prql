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
        $this->assertFileExists("lib/libprql_lib.so");
    }

    public function testPrqlHeaderFileExists(): void
    {
        $this->assertFileExists("lib/libprql_lib.h");
    }

    public function testInvalidQuery(): void
    {
        $prql = new Compiler();
        $res = $prql->compile("invalid");

        $this->assertCount(1, $res->messages);
    }

    public function testCompileWorks(): void
    {
        $options = new Options();
        $options->format = false;
        $options->signature_comment = false;
        $options->target = "sql.mssql";
        $prql = new Compiler();

        $actual = $prql->compile("from employees | take 10", $options);
        $expected = "SELECT TOP (10) * FROM employees";

        $this->assertEquals($expected, $actual->output);
    }

    public function testOtherFunctions(): void
    {
        $prql = new Compiler();

        $query = "let a = (from employees | take 10)\n\nfrom a | select [first_name]";

        $pl = $prql->prqlToPL($query);
        $rq = $prql->plToRQ($pl->output);
        $though_json = $prql->rqToSQL($rq->output);

        $direct = $prql->compile($query);

        $this->assertEquals($though_json, $direct);
    }
}
