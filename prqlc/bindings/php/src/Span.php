<?php

declare(strict_types=1);

namespace Prql\Compiler;

/**
 * Identifier of a location in source.
 * Contains offsets in terms of chars.
 */
final class Span
{
    /**
     * Start offset.
     */
    public int $start;

    /**
     * End offset.
     */
    public int $end;
}
