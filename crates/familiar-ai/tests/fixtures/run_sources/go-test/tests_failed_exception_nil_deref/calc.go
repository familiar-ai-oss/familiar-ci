package calc

import "strconv"

type box struct{ v int }

func Add(a, b int) int {
	return a + b
}

func Parse(s string) int {
	var b *box
	n, _ := strconv.Atoi(s)
	return n + b.v
}
