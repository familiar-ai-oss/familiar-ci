package calc

import "testing"

func TestParse(t *testing.T) {
	if got := Parse("x1"); got != 1 {
		t.Errorf("Parse = %d, want 1", got)
	}
}
