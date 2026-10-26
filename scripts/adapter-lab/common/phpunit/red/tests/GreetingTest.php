<?php

declare(strict_types=1);

require_once __DIR__ . '/../src/Greeter.php';

use App\Greeter;
use PHPUnit\Framework\TestCase;

final class GreetingTest extends TestCase
{
    public function testGreetSaysHello(): void
    {
        $this->assertSame('hello', Greeter::greet());
    }
}
