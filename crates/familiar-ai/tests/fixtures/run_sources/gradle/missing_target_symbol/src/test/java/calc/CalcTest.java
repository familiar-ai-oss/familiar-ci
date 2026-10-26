package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class CalcTest {
    @Test
    void multiplies() {
        assertEquals(6, Calc.mul(2, 3));
    }
}
