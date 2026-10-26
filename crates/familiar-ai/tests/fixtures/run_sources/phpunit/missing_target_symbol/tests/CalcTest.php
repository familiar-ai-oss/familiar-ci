<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';

use App\Calc;
use PHPUnit\Framework\TestCase;

final class CalcTest extends TestCase
{
    public function testMul(): void
    {
        $this->assertSame(6, Calc::mul(2, 3));
    }
}
