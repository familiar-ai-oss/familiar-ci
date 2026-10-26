package calc

import "testing"

func TestAdd(t *testing.T) {
	if got := Add(2, 2); got != 4 {
		t.Errorf("Add(2, 2) = %d, want 4", got)
	}
