<?php

declare(strict_types=1);

namespace Prql\Compiler;

/**
 * Result of compilation.
 */
final class Result
{
    /**
     * @var string
     */
    public string $output;

    /**
     * @var array<Message>
     */
    public array $messages;
}
