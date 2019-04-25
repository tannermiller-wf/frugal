// +build js

package frugal

import "github.com/gopherjs/gopherjs/js"

type safeLogger struct{}

func logger() *safeLogger {
	return &safeLogger{}
}

func (log *safeLogger) Error(msg string) {
	js.Global.Get("console").Call("error", msg)
}

func (log *safeLogger) Debug(msg string) {
	js.Global.Get("console").Call("debug", msg)
}

func (log *safeLogger) Warn(msg string) {
	js.Global.Get("console").Call("warn", msg)
}

func (log *safeLogger) Print(msg string) {
	js.Global.Get("console").Call("log", msg)
}

func (log *safeLogger) Info(msg string) {
	js.Global.Get("console").Call("info", msg)
}

var generateCorrelationID = func() string {
	return s4() + s4() + s4() + s4() + s4() + s4() + s4() + s4()
}

func s4() string {
	val := (js.Global.Get("Math").Call("random").Float() + 1) * 0x10000
	return js.Global.Get("Math").Call("floor", val).Call("toString", 16).Call("substring", 1).String()
}

var getNextOpID = func() string {
	nextOpID++
	return js.InternalObject(float64(nextOpID)).Call("toString").String()
}
