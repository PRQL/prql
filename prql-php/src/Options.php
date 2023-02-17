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
class Options
{
    /**
     * Pass generated SQL string trough a formatter that splits it into
     * multiple lines and prettifies indentation and spacing.
     *
     * @var bool
     */
    public bool $format = true;

    /**
     * Target and dialect to compile to.
     *
     * @var string|null
     */
    public ?string $target;

    /**
     * Emits the compiler signature as a comment after generated SQL.
     *
     * @var bool
     */
    public bool $signature_comment = true;
}
