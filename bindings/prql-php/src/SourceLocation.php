<?php

declare(strict_types=1);

namespace Prql\Compiler;

/**
 * Location within a source file.
 */
final class SourceLocation
{
    /**
     * Start line.
     */
    public int $start_line;

    /**
     * Start column.
     */
    public int $start_col;

    /**
     * End line.
     */
    public int $end_line;

    /**
     * End column.
     */
    public int $end_col;
}
