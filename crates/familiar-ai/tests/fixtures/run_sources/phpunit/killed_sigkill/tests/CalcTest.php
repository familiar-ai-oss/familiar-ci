<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';

use App\Calc;
use PHPUnit\Framework\TestCase;

final class CalcTest extends TestCase
{
    public function testCrash(): void
    {
        posix_kill(getmypid(), 9); sleep(5);
    }
}
