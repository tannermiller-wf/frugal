// +build !js

package thrift

import "encoding/json"

func jsonQuote(s string) string {
	b, _ := json.Marshal(s)
	s1 := string(b)
	return s1
}

func jsonUnquote(s string) (string, bool) {
	s1 := new(string)
	err := json.Unmarshal([]byte(s), s1)
	return *s1, err == nil
}
