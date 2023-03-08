<?php

declare(strict_types=1);

namespace Prql\Compiler;

/**
 * Compile result message.
 */
final class Message
{
    /**
     * Message kind. Currently only Error is implemented.
     */
    public MessageKind $kind;
    /**
     * Machine-readable identifier of the error.
     */
    public ?string $code;
    /**
     * Plain text of the error.
     */
    public string $reason;
    /**
     * A list of suggestions of how to fix the error.
     */
    public ?string $hint;
    /**
     * Character offset of error origin within a source file.
     */
    public ?Span $span;
    /**
     * Annotated code, containing cause and hints.
     */
    public ?string $display;
    /**
     * Line and column number of error origin within a source file.
     */
    public ?SourceLocation $location;
}
