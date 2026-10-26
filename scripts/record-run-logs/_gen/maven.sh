. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/lib.sh
IMG=maven:3.9-eclipse-temurin-21
D_() { printf "../../_bin/dock $IMG '%s'" "$1"; }
pom() { mk pom.xml <<'X'
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd">
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>calc</artifactId>
  <version>1.0</version>
  <properties>
    <maven.compiler.release>21</maven.compiler.release>
    <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
  </properties>
  <dependencies>
    <dependency>
      <groupId>org.junit.jupiter</groupId>
      <artifactId>junit-jupiter</artifactId>
      <version>5.11.4</version>
      <scope>test</scope>
    </dependency>
    <!--EXTRA-->
  </dependencies>
  <build>
    <plugins>
      <plugin>
        <groupId>org.apache.maven.plugins</groupId>
        <artifactId>maven-surefire-plugin</artifactId>
        <version>3.5.2</version>
      </plugin>
    </plugins>
  </build>
</project>
X
}
base() {
  fx maven "$1"
  pom
  mk src/main/java/calc/Calc.java <<'X'
package calc;

public class Calc {
    public static int add(int a, int b) {
        return a + b;
    }

    public static int parse(String s) {
        return Integer.parseInt(s);
    }
}
X
  setpre 'rm -rf target report'
  setver "$(D_ 'mvn --version')"
  setcmd "$(D_ 'mvn test')"
}
report() { printf '%s\n' 'mkdir -p report && cp -r target/surefire-reports/. report/ 2>/dev/null; true' > post.sh; }
okt() { mk src/test/java/calc/CalcTest.java <<'X'
package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class CalcTest {
    @Test
    void adds() {
        assertEquals(4, Calc.add(2, 2));
    }
}
X
}
base passed; okt; rec
base tests_failed; report; mk src/test/java/calc/CalcTest.java <<'X'
package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class CalcTest {
    @Test
    void adds() {
        assertEquals(5, Calc.add(2, 2));
    }
}
X
rec
base tests_failed_exception; report; mk src/test/java/calc/CalcTest.java <<'X'
package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class CalcTest {
    @Test
    void parses() {
        assertEquals(1, Calc.parse("x1"));
    }
}
X
rec
base collection_error; mk src/test/java/calc/CalcTest.java <<'X'
package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class CalcTest {
    @Test
    void adds() {
        assertEquals(4, Calc.add(2, 2))
    }
}
X
rec
base collection_error_import; mk src/test/java/calc/CalcTest.java <<'X'
package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;
import org.nosuch.Helper;

class CalcTest {
    @Test
    void adds() {
        assertEquals(4, Calc.add(2, 2));
    }
}
X
rec
base missing_target_symbol; mk src/test/java/calc/CalcTest.java <<'X'
package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class CalcTest {
    @Test
    void multiplies() {
        assertEquals(6, Calc.mul(2, 3));
    }
}
X
rec
base no_tests_collected_file; mk src/test/java/calc/CalcTest.java <<'X'
package calc;

class CalcTest {
    int helper() {
        return Calc.add(1, 1);
    }
}
X
rec
base no_tests_collected_filter; okt; setcmd "$(D_ 'mvn test -Dtest=Nomatch')"; rec
base setup_error; report; mk src/test/java/calc/CalcTest.java <<'X'
package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;

class CalcTest {
    @BeforeAll
    static void connect() {
        throw new IllegalStateException("db down");
    }

    @Test
    void adds() {
        assertEquals(4, Calc.add(2, 2));
    }
}
X
rec
base usage_error; okt; setcmd "$(D_ 'mvn test --nosuchflag')"; rec
base environment_error_cmd_not_found; okt; setcmd 'mvn test'; setver 'command -v mvn || echo "mvn: not installed on this host (the point of this fixture)"'; rec
base environment_error_missing_dep; okt; sed -i 's#<!--EXTRA-->#<dependency><groupId>com.example.nosuch</groupId><artifactId>nosuch-helper</artifactId><version>9.9.9</version></dependency>#' pom.xml
setpre 'rm -rf ../../.cache/m2/com/example/nosuch  # forget the cached resolution failure: first-attempt form'; rec
base environment_error_missing_dep_cached; okt; sed -i 's#<!--EXTRA-->#<dependency><groupId>com.example.nosuch</groupId><artifactId>nosuch-helper</artifactId><version>9.9.9</version></dependency>#' pom.xml
printf '%s\n' '# runs after environment_error_missing_dep (glob order): maven has cached that failure in .cache/m2' >> pre.sh; rec
base runner_crash; okt; mk pom.xml <<'X'
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
  <modelVersion>4.0.0</modelVersion>
  <groupId>com.example</groupId>
  <artifactId>calc</artifactId>
  <version>1.0
  <dependencies>
</project>
X
rec
for k in segv segv_shell abort sigkill exit; do
  base killed_$k; report
  case $k in segv|segv_shell) body='kill("SEGV");';; abort) body='kill("ABRT");';; sigkill) body='kill("KILL");';; exit) body='System.exit(3);';; esac
  mk src/test/java/calc/CalcTest.java <<X
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
        $body
    }
}
X
  [ $k = segv_shell ] && setcmd "$(D_ 'mvn test'); exit \$?"
  rec
done
base big_run; report; mk src/test/java/calc/CalcTest.java <<'X'
package calc;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.util.stream.IntStream;

import org.junit.jupiter.params.ParameterizedTest;
import org.junit.jupiter.params.provider.MethodSource;

class CalcTest {
    static IntStream numbers() {
        return IntStream.range(0, 600);
    }

    @ParameterizedTest
    @MethodSource("numbers")
    void adds(int i) {
        assertEquals(i == 300 ? -1 : i + 1, Calc.add(i, 1));
    }
}
X
rec
