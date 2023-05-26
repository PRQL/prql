<?php

declare(strict_types=1);

namespace Prql\Compiler\Test;

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
        $this->assertCount(0, $actual->messages);

        $this->assertEquals("SELECT TOP (10) * FROM employees", $actual->output);
    }

    public function testOtherFunctions(): void
    {
        $prql = new Compiler();

        $query = "
            let a = (from employees | take 10)

            from a | select {first_name}
        ";

        $pl = $prql->prqlToPL($query);
        $this->assertCount(0, $pl->messages);

        $rq = $prql->plToRQ($pl->output);
        $this->assertCount(0, $rq->messages);

        $via_json = $prql->rqToSQL($rq->output);
        $this->assertCount(0, $via_json->messages);

        $direct = $prql->compile($query);
        $this->assertCount(0, $direct->messages);

        $this->assertEquals($via_json, $direct);
    }
}
