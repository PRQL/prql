<?php

declare(strict_types=1);

namespace Prql\Compiler;

/**
 * Identifier of a location in source.
 * Contains offsets in terms of chars.
 */
final class Span
{
    public int $start;
    public int $end;
}
