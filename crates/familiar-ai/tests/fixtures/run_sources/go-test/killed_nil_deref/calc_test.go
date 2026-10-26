package calc

import "testing"

type box struct{ v int }

func TestNil(t *testing.T) {
	var b *box
	if b.v != 0 {
		t.Fail()
	}
}
