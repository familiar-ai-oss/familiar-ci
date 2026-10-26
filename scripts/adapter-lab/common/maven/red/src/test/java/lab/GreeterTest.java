package lab;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class GreeterTest {
    @Test
    void greetSaysHello() {
        assertEquals("hello", Greeter.greet());
    }
}
