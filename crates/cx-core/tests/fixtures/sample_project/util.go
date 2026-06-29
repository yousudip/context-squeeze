// Package util provides small helpers.
package util

import "fmt"

// Greeter holds a greeting prefix.
type Greeter struct {
	Prefix string
}

// Greet returns a greeting for name.
func (g Greeter) Greet(name string) string {
	return fmt.Sprintf("%s, %s!", g.Prefix, name)
}

// Max returns the larger of a and b.
func Max(a, b int) int {
	if a > b {
		return a
	}
	return b
}
