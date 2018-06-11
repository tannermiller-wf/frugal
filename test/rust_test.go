/*
 * Copyright 2018 Workiva
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *     http://www.apache.org/licenses/LICENSE-2.0
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package test

import (
	"fmt"
	"path/filepath"
	"testing"

	"github.com/Workiva/frugal/compiler"
)

func TestValidRustFrugalCompiler(t *testing.T) {
	options := compiler.Options{
		File:    frugalGenFile,
		Gen:     "rust",
		Out:     outputDir,
		Delim:   delim,
		Recurse: true,
	}
	if err := compiler.Compile(options); err != nil {
		fmt.Printf("err: %#v\n", err)
		t.Fatal("Unexpected error", err)
	}

	files := []FileComparisonPair{
		{"expected/rust/actual_base_rust/src/lib.rs", filepath.Join(outputDir, "actual_base_rust", "src", "lib.rs")},
		{"expected/rust/actual_base_rust/src/basefoo_service.rs", filepath.Join(outputDir, "actual_base_rust", "src", "basefoo_service.rs")},

		{"expected/rust/variety/src/lib.rs", filepath.Join(outputDir, "variety", "src", "lib.rs")},
	}
	copyAllFiles(t, files)
	compareAllFiles(t, files)
}
