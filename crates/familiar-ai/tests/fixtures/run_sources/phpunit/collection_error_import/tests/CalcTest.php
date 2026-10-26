<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';
require_once __DIR__ . '/../src/NosuchHelper.php';

use App\Calc;
use PHPUnit\Framework\TestCase;

final class CalcTest extends TestCase
{
    public function testAdd(): void
    {
        $this->assertSame(4, Calc::add(2, 2));
    }
}
