package lab

import "testing"

func TestGreetSaysHello(t *testing.T) {
	if got := Greet(); got != "hello" {
		t.Errorf("Greet() = %q, want %q", got, "hello")
	}
}
