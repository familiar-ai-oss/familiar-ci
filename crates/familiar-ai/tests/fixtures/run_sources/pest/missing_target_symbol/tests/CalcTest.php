<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';

use App\Calc;

test('Mul', function () {
    expect(Calc::mul(2, 3))->toBe(6);
});
