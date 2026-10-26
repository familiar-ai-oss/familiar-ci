<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Calc.php';

use App\Calc;

test('Crash', function () {
    posix_kill(getmypid(), 6); sleep(5);
});
