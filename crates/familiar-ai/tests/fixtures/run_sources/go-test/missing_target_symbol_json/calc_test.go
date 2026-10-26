package calc

import "testing"

func TestMul(t *testing.T) {
	if got := Mul(2, 3); got != 6 {
		t.Errorf("Mul(2, 3) = %d, want 6", got)
	}
}
