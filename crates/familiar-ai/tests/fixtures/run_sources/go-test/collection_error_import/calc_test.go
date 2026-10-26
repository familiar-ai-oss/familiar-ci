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
