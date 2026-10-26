<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';

use App\Calc;

test('Add', function () {
    expect(Calc::add(2, 2))->toBe(4)
});
