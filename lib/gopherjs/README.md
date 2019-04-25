# GopherJS Frugal Compile Target

### Story
TL;DR: transpiled frugal code (that imports a constant) creates an 8MB JS application; let’s not do that.

GopherJS is a tool that allows the compilation of go code to javascript in order to share critical application logic between clients and servers.  This compilation from go to js is referred to as `transpiling` for the remainder of this document.  This process of transpiling application code has become a critical component of go application development enabling complex logic to be written once, but used by both clients and servers. As such, we would like to provide better transpilation performance with regards to generated code size.

While working with gopherjs for the past few years, we have identified imports as the primary reason for large transpiled applications.  Due to the way imports are used in go, gopherjs is unable to "shake" import trees, and thus must generate all public types and types referenced in package `init`s within the entire import tree.  Effects of import optimization can be found in a similar series of changes made here: https://github.com/gopherjs/websocket/pull/20.

Currently, thrift and frugal import net/http which brings in an entire http serving library along with TLS encryption and many other fairly large packages that are unnecessary when running an application in javascript.  To mitigate these import trees, thrift and frugal go libraries have been vendored and modified to import less things.  Additionally, where possible, the build flag `// +build !js` has been added to things not needed at all in client logic, such as `simple_server.go` along with a few others.  Finally, a few isomorphic functions have been put in place in order to enable smaller generation of transpiled code.

### Design Notes
To accommodate for gopherjs being a first class consumer of frugal, we should make a new compile target for it that allows us to optimize the resulting code.  Currently, this target configures some options and essentially runs the existing golang compiler.  This should get us started quickly and allow us to extend this new target in the future.

As noted above, imports can drastically increase the size of a transpiled application.  As such, a new frugal library and vendored copy of thrift packages have been included in this directory that allows us to squeeze further transpiling performance out of them.

Finally, while testing I found a bug with const generation that references external types.  I wrote a little fix for that and included that in the normal go parser as well.  This should allow us to rename persisted “expected” files from \*.txt to \*.go as they should compile correctly now (see example with new gopherjs tests).

### Test Results
Using three different test cases we can see the effects of this change.  Below are the files I used to test the various transpiled results from thrift.

```sh
$ tail -n +1 *
==> gopher.go <==
package main

import "github.com/Workiva/frugal/test/expected/gopherjs/variety"

func main() {
        println(variety.DEFAULT_ID)
}

==> master.go <==
package main

import "github.com/Workiva/frugal/test/out/variety"

func main() {
        println(variety.DEFAULT_ID)
}

==> slim.go <==
package main

import "github.com/Workiva/frugal/test/out/variety"

func main() {
        println(variety.DEFAULT_ID)
}

==> test.sh <==
#! /bin/sh

go test -run=TestValidGoFrugalCompiler github.com/Workiva/frugal/test > /dev/null
gopherjs build -o test0-master.js master.go
gopherjs build -m -o test0-master.min.js master.go

go test -run=TestSlim github.com/Workiva/frugal/test > /dev/null
gopherjs build -o test1-slim.js slim.go
gopherjs build -m -o test1-slim.min.js slim.go

go test -run=TestValidGopherjsFrugalCompiler github.com/Workiva/frugal/test > /dev/null
gopherjs build -o test2-gopher.js gopher.go
gopherjs build -m -o test2-gopher.min.js gopher.go

ls -l *.js | awk '{print $5 "\t" $9}'
rm *.js*
```

Which gives the following results

```
$ ./test.sh
8470279 test0-master.js
5492268 test0-master.min.js
8302420 test1-slim.js
5382206 test1-slim.min.js
1128616 test2-gopher.js
746860  test2-gopher.min.js
```

Or tabular

  | Standard Go | Slim Go | GopherJS
-- | -- | -- | --
Normal | 8470279 (8.4 MB) | 8302420 (8.3 MB) | 1128616 (1.1 MB)
Minified | 5492268 (5.5 MB) | 5382206 (5.3 MB) | 746860 (746 kB)
