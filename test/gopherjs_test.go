package test

import (
	"testing"

	"github.com/Workiva/frugal/compiler"
)

var gopherjsVerifyFiles = []string{
	"actual_base/golang/f_basefoo_service.go",
	"actual_base/golang/f_types.go",
	"intermediate_include/f_intermediatefoo_service.go",
	"intermediate_include/f_types.go",
	"subdir_include/f_types.go",
	"validStructs/f_types.go",
	"ValidTypes/f_types.go",
	"variety/f_events_scope.go",
	"variety/f_foo_service.go",
	"variety/f_footransitivedeps_service.go",
	"variety/f_types.go",
}

func TestValidGopherjsFrugalCompiler(t *testing.T) {
	options := compiler.Options{
		File:    frugalGenFile,
		Gen:     "gopherjs:package_prefix=github.com/Workiva/frugal/test/expected/gopherjs/",
		Out:     outputDir,
		Delim:   delim,
		Recurse: true,
	}
	if err := compiler.Compile(options); err != nil {
		t.Fatal("Unexpected error", err)
	}
	files := make([]FileComparisonPair, len(gopherjsVerifyFiles))
	for i, file := range gopherjsVerifyFiles {
		files[i].ExpectedPath = "expected/gopherjs/" + file
		files[i].GeneratedPath = outputDir + "/" + file
	}
	copyAllFiles(t, files)
	compareAllFiles(t, files)
}
