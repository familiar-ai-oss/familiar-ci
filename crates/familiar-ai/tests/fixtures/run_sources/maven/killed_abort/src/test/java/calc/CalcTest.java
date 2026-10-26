package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class CalcTest {
    static void kill(String sig) throws Exception {
        long pid = ProcessHandle.current().pid();
        new ProcessBuilder("sh", "-c", "kill -" + sig + " " + pid).inheritIO().start().waitFor();
        Thread.sleep(5000);
    }

    @Test
    void adds() {
        assertEquals(4, Calc.add(2, 2));
    }

    @Test
    void crashes() throws Exception {
        kill("ABRT");
    }
}
