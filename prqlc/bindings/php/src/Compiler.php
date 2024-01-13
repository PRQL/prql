<?php

/**
 * PRQL compiler bindings.
 *
 * This library requires the PHP FFI extension.
 * It also requires the libprqlc_c library.
 *
 * PHP version 8.0
 *
 * @api
 *
 * @author    PRQL
 * @copyright 2023 PRQL
 * @license   https://spdx.org/licenses/Apache-2.0.html Apache License 2.0
 *
 * @link https://prql-lang.org/
 */

declare(strict_types=1);

namespace Prql\Compiler;

/**
 * PRQL Compiler.
 *
 * @author  PRQL
 * @license https://spdx.org/licenses/Apache-2.0.html Apache License 2.0
 *
 * @link https://prql-lang.org/
 */
final class Compiler
{
    private static \FFI $ffi;

    /**
     * Initializes a new instance of the Compiler.
     *
     * @param string|null $lib_path path to the libprql library
     */
    public function __construct(?string $lib_path = null)
    {
        if (isset(self::$ffi)) {
            return;
        }

        if ($lib_path === null) {
            $lib_path = __DIR__ . '/../lib';
        }

        $header = $lib_path . '/libprqlc_c.h';

        if (PHP_OS_FAMILY === 'Windows') {
            $library = $lib_path . "\libprqlc_c.dll";
        } elseif (PHP_OS_FAMILY === 'Darwin') {
            $library = $lib_path . '/libprqlc_c.dylib';
        } else {
            $library = $lib_path . '/libprqlc_c.so';
        }

        $header_source = file_get_contents($header, false, null, 0, 1024 * 1024);

        if ($header_source === false) {
            throw new \InvalidArgumentException('Cannot load header file.');
        }

        self::$ffi = \FFI::cdef($header_source, $library);
    }

    /**
     * Compile a PRQL string into a SQL string.
     *
     * @param string       $prql_query a PRQL query
     * @param Options|null $options    compile options
     *
     * @return Result compilation result containing SQL query
     *
     * @throws \InvalidArgumentException on NULL input
     */
    public function compile(string $prql_query, ?Options $options = null): Result
    {
        if (!$prql_query) {
            throw new \InvalidArgumentException('No query given.');
        }

        $ffi_options = $this->optionsInit($options);

        $res = self::$ffi->compile($prql_query, \FFI::addr($ffi_options));

        $this->optionsDestroy($ffi_options);

        return $this->convertResult($res);
    }

    /**
     * Compile a PRQL string into PL.
     *
     * @param string $prql_query PRQL query
     *
     * @return Result compilation result containing PL serialized as JSON
     *
     * @throws \InvalidArgumentException on NULL input
     *
     * @api
     */
    public function prqlToPL(string $prql_query): Result
    {
        if (!$prql_query) {
            throw new \InvalidArgumentException('No query given.');
        }

        $res = self::$ffi->prql_to_pl($prql_query);

        return $this->convertResult($res);
    }

    /**
     * Converts PL to RQ.
     *
     * @param string $pl_json PL serialized as JSON
     *
     * @return Result compilation result containing RQ serialized as JSON
     *
     * @throws \InvalidArgumentException on NULL input
     *
     * @api
     */
    public function plToRQ(string $pl_json): Result
    {
        if (!$pl_json) {
            throw new \InvalidArgumentException('No query given.');
        }

        $res = self::$ffi->pl_to_rq($pl_json);

        return $this->convertResult($res);
    }

    /**
     * Converts RQ to SQL.
     *
     * @param string       $rq_json RQ serialized as JSON
     * @param Options|null $options compile options
     *
     * @return Result compilation result containing SQL query
     *
     * @throws \InvalidArgumentException on NULL input
     *
     * @api
     */
    public function rqToSQL(string $rq_json, ?Options $options = null): Result
    {
        if (!$rq_json) {
            throw new \InvalidArgumentException('No query given.');
        }

        $ffi_options = $this->optionsInit($options);

        $res = self::$ffi->rq_to_sql($rq_json, \FFI::addr($ffi_options));

        $this->optionsDestroy($ffi_options);

        return $this->convertResult($res);
    }

    private function optionsInit(?Options $options = null)
    {
        if ($options === null) {
            $options = new Options();
        }

        $ffi_options = self::$ffi->new('struct Options');
        $ffi_options->format = $options->format;
        $ffi_options->signature_comment = $options->signature_comment;

        if (isset($options->target)) {
            $len = strlen($options->target) + 1;
            $ffi_options->target = \FFI::new("char[$len]", false);
            \FFI::memcpy($ffi_options->target, $options->target, $len - 1);
        }

        return $ffi_options;
    }

    private function optionsDestroy($ffi_options)
    {
        if (!\FFI::isNull($ffi_options->target)) {
            \FFI::free($ffi_options->target);
        }

        unset($ffi_options);
    }

    private function convertResult($ffi_res): Result
    {
        $res = new Result();

        // convert string
        $res->output = $ffi_res->output;


        $res->messages = [];
        for ($i = 0; $i < $ffi_res->messages_len; ++$i) {
            $res->messages[$i] = $this->convertMessage($ffi_res->messages[$i]);
        }

        // free the ffi_result
        self::$ffi->result_destroy($ffi_res);

        return $res;
    }

    private function convertMessage($ffi_msg): Message
    {
        $msg = new Message();

        // I'm using numbers here, I cannot find a way to refer to MessageKind.Error
        if ($ffi_msg->kind == 0) {
            $msg->kind = MessageKind::Error;
        } elseif ($ffi_msg->kind == 1) {
            $msg->kind = MessageKind::Warning;
        } elseif ($ffi_msg->kind == 2) {
            $msg->kind = MessageKind::Lint;
        }

        $msg->code = $this->convertNullableString($ffi_msg->code);
        $msg->reason = $ffi_msg->reason;
        $msg->span = $this->convertSpan($ffi_msg->span);
        $msg->hint = $this->convertNullableString($ffi_msg->hint);

        $msg->display = $this->convertNullableString($ffi_msg->display);
        $msg->location = $this->convertLocation($ffi_msg->location);

        return $msg;
    }

    private function convertSpan($ffi_ptr): ?Span
    {
        if (is_null($ffi_ptr) || \FFI::isNull($ffi_ptr)) {
            return null;
        }

        $span = new Span();
        $span->start = $ffi_ptr[0]->start;
        $span->end = $ffi_ptr[0]->end;

        return $span;
    }

    private function convertLocation($ffi_ptr): ?SourceLocation
    {
        if (is_null($ffi_ptr) || \FFI::isNull($ffi_ptr)) {
            return null;
        }

        $location = new SourceLocation();
        $location->start_line = $ffi_ptr[0]->start_line;
        $location->start_col = $ffi_ptr[0]->start_col;
        $location->end_line = $ffi_ptr[0]->end_line;
        $location->end_col = $ffi_ptr[0]->end_col;

        return $location;
    }

    private function convertNullableString($ffi_ptr): ?string
    {
        if (is_null($ffi_ptr) || \FFI::isNull($ffi_ptr)) {
            return null;
        }
        return $ffi_ptr[0];
    }
}
