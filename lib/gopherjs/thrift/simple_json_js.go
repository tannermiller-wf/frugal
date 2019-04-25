// +build js

package thrift

import "github.com/gopherjs/gopherjs/js"

func jsonQuote(s string) string {
	return js.Global.Get("JSON").Call("stringify", js.InternalObject(s)).String()
}

func jsonUnquote(s string) (string, bool) {
	return js.Global.Get("JSON").Call("parse", js.InternalObject(s)).String(), true
}
