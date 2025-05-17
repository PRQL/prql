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
 * @see https://prql-lang.org/
 */

declare(strict_types=1);

namespace Prql\Compiler;

/**
 * Compilation options for SQL backend of the compiler.
 *
 * @author  PRQL
 * @license https://spdx.org/licenses/Apache-2.0.html Apache License 2.0
 *
 * @see https://prql-lang.org/
 */
final class Options
{
    /**
     * Pass generated SQL string trough a formatter that splits it into
     * multiple lines and prettifies indentation and spacing.
     */
    public bool $format = true;

    /**
     * Target and dialect to compile to.
     */
    public ?string $target = null;

    /**
     * Emits the compiler signature as a comment after generated SQL.
     */
    public bool $signature_comment = true;
}
