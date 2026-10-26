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
