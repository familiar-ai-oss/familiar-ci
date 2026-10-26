<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';

use App\Calc;

test('Parse', function () {
    expect(Calc::parse("x1"))->toBe(1);
});
