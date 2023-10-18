<?php

declare(strict_types=1);

namespace Prql\Compiler;

/**
 * Compile message kind. Currently only Error is implemented.
 */
enum MessageKind
{
    /**
     * Error message.
     */
    case Error;
    /**
     * Warning message.
     */
    case Warning;
    /**
     * Lint message.
     */
    case Lint;
}
