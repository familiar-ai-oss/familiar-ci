<?php

declare(strict_types=1);

namespace App;

final class Calc
{
    public static function add(int $a, int $b): int
    {
        return $a + $b;
    }

    public static function parse(string $s): int
    {
        if (preg_match('/^\d+$/', $s) !== 1) {
            throw new \InvalidArgumentException("not a number: $s");
        }
        return (int) $s;
    }
}
