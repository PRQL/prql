<?php

declare(strict_types=1);

namespace Prql\Compiler;

/**
 * Location within a source file.
 */
final class SourceLocation
{
    public int $start_line;
    public int $start_col;
    public int $end_line;
    public int $end_col;
}
