<?php

/**
 * PRQL compiler bindings.
 *
 * This library requires the PHP FFI extension.
 * It also requires the libprql_lib library.
 *
 * PHP version 8.0
 *
 * @api
 * @package   Prql\Compiler
 * @author    PRQL
 * @copyright 2023 PRQL
 * @license   https://spdx.org/licenses/Apache-2.0.html Apache License 2.0
 * @link      https://prql-lang.org/
 */

declare(strict_types=1);

namespace Prql\Compiler;

/**
 * The PRQL compiler transpiles PRQL queries.
 *
 * @package Prql\Compiler
 * @author  PRQL
 * @license https://spdx.org/licenses/Apache-2.0.html Apache License 2.0
 * @link    https://prql-lang.org/
 */
final class Compiler
{
    private \FFI $_libprql;

    /**
     * Initializes a new instance of the Compiler.
     *
     * @param ?string|null $lib_path Path to the libprql library.
     */
    function __construct(?string $lib_path = null)
    {
        if ($lib_path === null) {
            $lib_path = __DIR__ . "/../lib";
        }

        $header = $lib_path . "/libprql_lib.h";

        if (PHP_OS_FAMILY === "Windows") {
            $library = $lib_path . "\libprql_lib.dll";
        } elseif (PHP_OS_FAMILY === "Darwin") {
            $library = $lib_path . "/libprql_lib.dylib";
        } else {
            $library = $lib_path . "/libprql_lib.so";
        }

        $header_source = file_get_contents($header, false, null, 0, 1024 * 1024);

        if ($header_source === false) {
            throw new \InvalidArgumentException("Cannot load header file.");
        }

        $this->_libprql = \FFI::cdef($header_source, $library);
    }

    /**
     * Compile a PRQL string into a SQL string.
     *
     * @param string       $prql_query A PRQL query.
     * @param Options|null $options    PRQL compiler options.
     *
     * @return Result compilation result containing SQL query.
     */
    function compile(string $prql_query, ?Options $options = null): Result
    {
        if (!$prql_query) {
            throw new \InvalidArgumentException("No query given.");
        }

        $ffi_options = $this->options_init($options);

        $res = $this->_libprql->compile($prql_query, \FFI::addr($ffi_options));

        $this->options_destroy($ffi_options);

        return $this->convert_result($res);
    }

    /**
     * Compile a PRQL string into PL.
     *
     * @param string $prql_query PRQL query.
     *
     * @return Result compilation result containing PL serialized as JSON.
     * @api
     */
    function prqlToPL(string $prql_query): Result
    {
        if (!$prql_query) {
            throw new \InvalidArgumentException("No query given.");
        }

        $res = $this->_libprql->prql_to_pl($prql_query);

        return $this->convert_result($res);
    }

    /**
     * Converts PL to RQ.
     *
     * @param string $pl_json PL serialized as JSON.
     *
     * @return Result compilation result containing RQ serialized as JSON.
     * @api
     */
    function plToRQ(string $pl_json): Result
    {
        if (!$pl_json) {
            throw new \InvalidArgumentException("No query given.");
        }

        $res = $this->_libprql->pl_to_rq($pl_json);

        return $this->convert_result($res);
    }

    /**
     * Converts RQ to SQL.
     *
     * @param string $rq_json PL in JSON format.
     *
     * @return Result compilation result containing SQL query.
     * @api
     */
    function rqToSQL(string $rq_json, ?Options $options = null): Result
    {
        if (!$rq_json) {
            throw new \InvalidArgumentException("No query given.");
        }

        $ffi_options = $this->options_init($options);

        $res = $this->_libprql->rq_to_sql($rq_json, $out);

        $this->options_destroy($ffi_options);

        return $this->convert_result($res);
    }

    private function options_init(?Options $options = null) {
        if ($options === null) {
            $options = new Options();
        }

        $ffi_options = $this->_libprql->new("struct Options");
        $ffi_options->format = $options->format;
        $ffi_options->signature_comment = $options->signature_comment;

        if (isset($options->target)) {
            $len = strlen($options->target) + 1;
            $ffi_options->target = \FFI::new("char[$len]", false);
            \FFI::memcpy($ffi_options->target, $options->target, $len - 1);
        }

        return $ffi_options;
    }

    private function options_destroy($ffi_options) {
        if (!\FFI::isNull($ffi_options->target)) {
            \FFI::free($ffi_options->target);
        }
        unset($ffi_options);
    }

    private function convert_result($ffi_res): Result {
        $res = new Result();

        // convert string
        $res->output = $this->convert_string($ffi_res->output);

        $res->messages = array();
        for ($i = 0; $i < $ffi_res->messages_len; $i++) {
            $res->messages[$i] = $this->convert_message($ffi_res->messages[$i]);
        }

        // free the ffi_result
        $this->_libprql->result_destroy($ffi_res);
        return $res;
    }

    private function convert_message($ffi_msg): Message {
        $msg = new Message();

        // I'm using numbers here, I cannot find a way to refer to MessageKind.Error
        if ($ffi_msg->kind == 0) {
            $msg->kind = MessageKind::Error;
        } else if ($ffi_msg->kind == 1) {
            $msg->kind = MessageKind::Warning;
        } else if ($ffi_msg->kind == 2) {
            $msg->kind = MessageKind::Lint;
        }

        $msg->code = $this->convert_nullable_string($ffi_msg->code);
        $msg->reason = $this->convert_string($ffi_msg->reason);
        $msg->span = $this->convert_span($ffi_msg->span);
        $msg->hint = $this->convert_nullable_string($ffi_msg->hint);

        $msg->display = $this->convert_nullable_string($ffi_msg->display);
        $msg->location = $this->convert_location($ffi_msg->location);

        return $msg;
    }

    private function convert_span($ffi_ptr): ?Span {
        if (is_null($ffi_ptr) || \FFI::isNull($ffi_ptr)) {
            return null;
        }
        $span = new Span();
        $span->start = $ffi_ptr[0]->start;
        $span->end = $ffi_ptr[0]->end;
        return $span;
    }

    private function convert_location($ffi_ptr): ?SourceLocation {
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

    private function convert_nullable_string($ffi_ptr): ?string {
        if (is_null($ffi_ptr) || \FFI::isNull($ffi_ptr)) {
            return null;
        }
        // dereference
        return $this->convert_string($ffi_ptr[0]);
    }

    private function convert_string($ffi_ptr): string {
        return \FFI::string(\FFI::cast(\FFI::type('char*'), $ffi_ptr));
    }
}
