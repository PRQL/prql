<?php

declare(strict_types=1);

namespace Prql\Compiler;

/**
 * Compile message kind. Currently only Error is implemented.
 */
enum MessageKind
{
    case Error;
    case Warning;
    case Lint;
}
