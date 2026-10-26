package calc

import "testing"

func TestParse(t *testing.T) {
	if got := Parse("12"); got != 12 {
		t.Errorf("Parse = %d, want 12", got)
	}
}
