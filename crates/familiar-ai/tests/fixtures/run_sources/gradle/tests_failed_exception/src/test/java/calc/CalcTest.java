package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class CalcTest {
    @Test
    void parses() {
        assertEquals(1, Calc.parse("x1"));
    }
}
