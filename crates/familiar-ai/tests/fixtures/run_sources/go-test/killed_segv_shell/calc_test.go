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
	syscall.Kill(syscall.Getpid(), syscall.SIGSEGV)
	time.Sleep(5 * time.Second)
}
