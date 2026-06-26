<?php

declare(strict_types=1);

namespace Prql\Compiler;

/**
 * Compile message kind.
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
