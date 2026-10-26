. /tmp/claude-1000/-home-user-projects-familiar-ai-oss/ce46f81f-ec40-4c7d-885d-547ef9d911b3/scratchpad/run_logs/_gen/lib.sh
IMG=golang:1.23-bookworm
D_() { printf "../../_bin/dock $IMG '%s'" "$1"; }
base() {
  fx go-test "$1"
  mk go.mod <<'X'
module example.com/calc

go 1.23
X
  mk calc.go <<'X'
package calc

import "strconv"

func Add(a, b int) int {
	return a + b
}

func Parse(s string) int {
	n, err := strconv.Atoi(s)
	if err != nil {
		panic(err)
	}
	return n
}
X
  setver "$(D_ 'go version')"
  setcmd "$(D_ 'go test ./...')"
}
okt() { mk calc_test.go <<'X'
package calc

import "testing"

func TestAdd(t *testing.T) {
	if got := Add(2, 2); got != 4 {
		t.Errorf("Add(2, 2) = %d, want 4", got)
	}
}
X
}
base passed; okt; rec
base tests_failed; mk calc_test.go <<'X'
package calc

import "testing"

func TestAdd(t *testing.T) {
	if got := Add(2, 2); got != 5 {
		t.Errorf("Add(2, 2) = %d, want 5", got)
	}
}
X
rec
base tests_failed_exception; mk calc_test.go <<'X'
package calc

import "testing"

func TestParse(t *testing.T) {
	if got := Parse("x1"); got != 1 {
		t.Errorf("Parse = %d, want 1", got)
	}
}
X
rec
base collection_error; mk calc_test.go <<'X'
package calc

import "testing"

func TestAdd(t *testing.T) {
	if got := Add(2, 2); got != 4 {
		t.Errorf("Add(2, 2) = %d, want 4", got)
	}
X
rec
base collection_error_import; mk calc_test.go <<'X'
package calc

import (
	"testing"

	"example.com/calc/nosuchpkg"
)

func TestAdd(t *testing.T) {
	nosuchpkg.Helper()
	if got := Add(2, 2); got != 4 {
		t.Errorf("Add(2, 2) = %d, want 4", got)
	}
}
X
rec
base missing_target_symbol; mk calc_test.go <<'X'
package calc

import "testing"

func TestMul(t *testing.T) {
	if got := Mul(2, 3); got != 6 {
		t.Errorf("Mul(2, 3) = %d, want 6", got)
	}
}
X
rec
base no_tests_collected_file; mk calc_test.go <<'X'
package calc

func helper() int {
	return Add(1, 1)
}
X
rec
base no_tests_collected_no_test_files; rec
base no_tests_collected_filter; okt; setcmd "$(D_ 'go test -run Nomatch ./...')"; rec
base setup_error; mk calc_test.go <<'X'
package calc

import (
	"fmt"
	"os"
	"testing"
)

func TestMain(m *testing.M) {
	fmt.Fprintln(os.Stderr, "setup: db down")
	os.Exit(1)
}

func TestAdd(t *testing.T) {
	if got := Add(2, 2); got != 4 {
		t.Errorf("Add(2, 2) = %d, want 4", got)
	}
}
X
rec
base usage_error; okt; setcmd "$(D_ 'go test -nosuchflag ./...')"; rec
base environment_error_cmd_not_found; okt; setcmd 'go test ./...'; setver 'command -v go || echo "go: not installed on this host (the point of this fixture)"'; rec
base environment_error_missing_dep; mk calc_test.go <<'X'
package calc

import (
	"testing"

	"github.com/nosuch/helperpkg"
)

func TestAdd(t *testing.T) {
	helperpkg.Helper()
	if got := Add(2, 2); got != 4 {
		t.Errorf("Add(2, 2) = %d, want 4", got)
	}
}
X
rec
base runner_crash; okt; mk go.mod <<'X'
module example.com/calc

go 1.23
require (
X
rec
for k in segv segv_shell abort sigkill; do
  base killed_$k
  case $k in segv|segv_shell) sig=SIGSEGV;; abort) sig=SIGABRT;; sigkill) sig=SIGKILL;; esac
  mk calc_test.go <<X
package calc

import (
	"syscall"
	"testing"
	"time"
)

func TestAdd(t *testing.T) {
	if got := Add(2, 2); got != 4 {
		t.Errorf("Add(2, 2) = %d, want 4", got)
	}
}

func TestCrash(t *testing.T) {
	syscall.Kill(syscall.Getpid(), syscall.$sig)
	time.Sleep(5 * time.Second)
}
X
  [ $k = segv_shell ] && setcmd "$(D_ 'go test ./...'); exit \$?"
  rec
done
base killed_nil_deref; mk calc_test.go <<'X'
package calc

import "testing"

type box struct{ v int }

func TestNil(t *testing.T) {
	var b *box
	if b.v != 0 {
		t.Fail()
	}
}
X
rec
base big_run
python3 - <<'X'
s = 'package calc\n\nimport "testing"\n\n'
for i in range(600):
    want = -1 if i == 300 else i + 1
    s += f'func TestAdd{i:03}(t *testing.T) {{\n\tif got := Add({i}, 1); got != {want} {{\n\t\tt.Errorf("Add({i}, 1) = %d, want {want}", got)\n\t}}\n}}\n\n'
open('calc_test.go', 'w').write(s)
X
rec
base big_run_verbose; cp ../big_run/calc_test.go . 2>/dev/null; setcmd "$(D_ 'go test -v ./...')"; rec
base passed_uncached; okt; setcmd "$(D_ 'go test -count=1 ./...')"; rec
