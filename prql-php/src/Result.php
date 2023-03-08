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
 * Result of compilation.
 */
final class Result
{
    public string $output;

    /**
     * @var array<Message>
     */
    public array $messages;
}

/**
 * Compile result message.
 */
final class Message {
    /**
     * Message kind. Currently only Error is implemented.
     */
    public MessageKind $kind;
    /**
     * Machine-readable identifier of the error
     */
    public ?string $code;
    /**
     * Plain text of the error
     */
    public string $reason;
    /**
     * A list of suggestions of how to fix the error
     */
    public ?string $hint;
    /**
     * Character offset of error origin within a source file
     */
    public ?Span $span;
    /**
     * Annotated code, containing cause and hints.
     */
    public ?string $display;
    /**
     * Line and column number of error origin within a source file
     */
    public ?SourceLocation $location;
}

/**
 * Compile message kind. Currently only Error is implemented.
 */
enum MessageKind
{
    case Error;
    case Warning;
    case Lint;
}

/**
 * Identifier of a location in source.
 * Contains offsets in terms of chars.
 */
final class Span
{
    public int $start;
    public int $end;
}

/**
 * Location within a source file.
 */
final class SourceLocation {
    public int $start_line;
    public int $start_col;
    public int $end_line;
    public int $end_col;
}