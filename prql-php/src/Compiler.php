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
        $library = $lib_path;

        if ($lib_path === null) {
            $library = __DIR__;
        }

        if (PHP_OS_FAMILY === "Windows") {
            $library .= "\libprql_lib.dll";
        } elseif (PHP_OS_FAMILY === "Darwin") {
            $library .= "/libprql_lib.dylib";
        } else {
            $library .= "/libprql_lib.so";
        }

        $this->_libprql = \FFI::cdef(
            "
            typedef struct Options {
                bool format;
                char *target;
                bool signature_comment;
            } Options;

            int compile(const char *prql_query, const struct Options *options, char *out);
            int prql_to_pl(const char *prql_query, char *out);
            int pl_to_rq(const char *pl_json, char *out);
            int rq_to_sql(const char *rq_json, char *out);
        ", $library
        );
    }

    /**
     * Compile a PRQL string into a SQL string.
     *
     * @param string       $prql_query A PRQL query.
     * @param Options|null $options    PRQL compiler options.
     *
     * @return string SQL query.
     * @throws \InvalidArgumentException If no query is given or the query canno
     * be compiled.
     * @api
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

        $ffi_options = $this->_libprql->new("struct Options");
        $ffi_options->format = $options->format;
        $ffi_options->signature_comment = $options->signature_comment;

        if (isset($options->target)) {
            $target_len = strlen($options->target);
            $ffi_options->target = \FFI::new("char[$target_len]", false);
            \FFI::memcpy($ffi_options->target, $options->target, $target_len);
            \FFI::free($ffi_options->target);
        }

        $out = str_pad("", 1024);
        if ($this->_libprql->compile($prql_query, \FFI::addr($ffi_options), $out) !== 0) {
            throw new \InvalidArgumentException("Could not compile query.");
        }

        unset($ffi_options);

        return trim($out);
    }

    /**
     * Compile a PRQL string into PL.
     *
     * @param string $prql_query A PRQL query.
     *
     * @return string Pipelined Language (PL) JSON string.
     * @throws \InvalidArgumentException If no query is given or the query cannot
     * be compiled.
     * @api
     */
    function prqlToPL(string $prql_query): string
    {
        if (!$prql_query) {
            throw new \InvalidArgumentException("No query given.");
        }

        $out = str_pad("", 1024);
        if ($this->_libprql->prql_to_pl($prql_query, $out) !== 0) {
            throw new \InvalidArgumentException("Could not compile query.");
        }

        return trim($out);
    }

    /**
     * Converts PL to RQ.
     *
     * @param string $pl_json PL in JSON format.
     *
     * @return string RQ string.
     * @throws \InvalidArgumentException If no query is given or the query cannot
     * be compiled.
     * @api
     */
    function pLToRQ(string $pl_json): string
    {
        if (!$prql_query) {
            throw new \InvalidArgumentException("No query given.");
        }

        $out = str_pad("", 1024);
        if ($this->_libprql->pl_to_rq($pl_json, $out) !== 0) {
            throw new \InvalidArgumentException("Could not convert PL.");
        }

        return trim($out);
    }

    /**
     * Converts RQ to SQL.
     *
     * @param string $rq_json PL in JSON format.
     *
     * @return string SQL string.
     * @throws \InvalidArgumentException If no query is given or the query cannot
     * be compiled.
     * @api
     */
    function rQToSql(string $rq_json): string
    {
        if (!$prql_query) {
            throw new \InvalidArgumentException("No query given.");
        }

        $out = str_pad("", 1024);
        if ($this->_libprql->rq_to_sql($rq_json, $out) !== 0) {
            throw new \InvalidArgumentException("Could not convert RQ.");
        }

        return trim($out);
    }
}
