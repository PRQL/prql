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
 * Compilation options for SQL backend of the compiler.
 *
 * @package Prql\Compiler
 * @author  PRQL
 * @license https://spdx.org/licenses/Apache-2.0.html Apache License 2.0
 * @link    https://prql-lang.org/
 */
final class Compiler
{
    /**
     * Compile a PRQL string into a SQL string.
     *
     * @param string       $prql_query A PRQL query.
     * @param Options|null $options    PRQL compiler options.
     *
     * @return string SQL query.
     * @throws \InvalidArgumentException If no query is given or the query cannot
     * @api
     * be compiled.
     * @todo   FIX THIS. THIS DOES NOT WORK!
     * @ignore Ignore this function until fixed.
     */
    function compile(string $prql_query, ?Options $options = null): string
    {
        if (!$prql_query) {
            throw new \InvalidArgumentException("No query given.");
        }

        if ($options === null) {
            $options = new Options();
        }

        $library = "/libprql_lib.so";

        if (PHP_OS_FAMILY === "Windows") {
            $library = "\libprql_lib.dll";
        }

        $libprql = \FFI::cdef(
            "
            struct options {
                bool format;
                char* target;
                bool signature_comment;
            };

            char* compile(const char *prql_query, struct options *opt);
        ", __DIR__ . $library
        );

        $ffi_options = $libprql->new("struct options");
        $ffi_options->format = $options->format;
        $ffi_options->signature_comment = $options->signature_comment;

        if (isset($options->target)) {
            $target_len = strlen($options->target);
            $ffi_options->target = $ffi->new('char[$target_len]', 0);
            FFI::memcpy($ffi_options->target, $options->target, $target_len);
            FFI::free($ffi_options->target);
        }

        $out = str_pad("", 1024);
        if ($libprql->compile($prql_query, \FFI::addr($$ffi_options)) !== 0) {
            throw new \InvalidArgumentException("Could not compile query.");
        }

        return trim($out);
    }

    /**
     * Compile a PRQL string into a JSON string.
     *
     * @param string $prql_query A PRQL query.
     *
     * @return string JSON string.
     * @throws \InvalidArgumentException If no query is given or the query cannot
     * be compiled.
     * @api
     */
    function toJson(string $prql_query): string
    {
        if (!$prql_query) {
            throw new \InvalidArgumentException("No query given.");
        }

        $library = "/libprql_lib.so";

        if (PHP_OS_FAMILY === "Windows") {
            $library = "\libprql_lib.dll";
        }

        $libprql = \FFI::cdef(
            "int to_json(char *prql_query, char *json_query);",
            __DIR__ . $library
        );

        $out = str_pad("", 1024);
        if ($libprql->to_json($prql_query, $out) !== 0) {
            throw new \InvalidArgumentException("Could not compile query.");
        }

        return trim($out);
    }

    /**
     * Compile a PRQL string into a SQL string.
     *
     * @param string $prql_query A PRQL query.
     *
     * @return string SQL query.
     * @throws \InvalidArgumentException If no query is given or the query cannot
     * be compiled.
     * @api
     */
    function toSql(string $prql_query): string
    {
        if (!$prql_query) {
            throw new \InvalidArgumentException("No query given.");
        }

        $library = "/libprql_lib.so";

        if (PHP_OS_FAMILY === "Windows") {
            $library = "\libprql_lib.dll";
        }

        $libprql = \FFI::cdef(
            "int to_sql(char *prql_query, char *sql_query);",
            __DIR__ . $library
        );

        $out = str_pad("", 1024);
        if ($libprql->to_sql($prql_query, $out) !== 0) {
            throw new \InvalidArgumentException("Could not compile query.");
        }

        return trim($out);
    }
}
