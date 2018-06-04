/*
 * Copyright 2017 Workiva
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

package rust

import (
	"bytes"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"unicode"

	"github.com/Workiva/frugal/compiler/generator"
	"github.com/Workiva/frugal/compiler/globals"
	"github.com/Workiva/frugal/compiler/parser"
)

// TODO: Run clippy on generated code to make sure its clean
// TODO: Implement annotations (somehow)

const (
	lang             = "rs"
	defaultOutputDir = "gen-rs"
	serviceSuffix    = "_service"
	scopeSuffix      = "_scope"

	packageTypeOption = "package_type"
)

type packageType string

const (
	packageTypeCrate  = "crate"
	packageTypeModule = "module"
)

func newPackageType(p string) packageType {
	switch p {
	case string(packageTypeModule):
		return packageTypeModule
	default:
		return packageTypeCrate
	}
}

func (p packageType) fileName() string {
	switch p {
	case packageTypeModule:
		return "mod"
	default:
		return "lib"
	}
}

func (p packageType) outputDir(outputDir string) string {
	switch p {
	case packageTypeModule:
		return outputDir
	default:
		return outputDir + "/src"
	}
}

func (p packageType) generateCargoTOML() bool {
	switch p {
	case packageTypeModule:
		return false
	default:
		return true
	}
}

type Generator struct {
	*generator.BaseGenerator
	rootFile    *os.File
	packageType packageType
}

func NewGenerator(options map[string]string) generator.LanguageGenerator {
	return &Generator{
		BaseGenerator: &generator.BaseGenerator{Options: options},
		rootFile:      nil,
		packageType:   newPackageType(options[packageTypeOption]),
	}
}

func (g *Generator) SetupGenerator(outputDir string) error {
	rootFile, err := g.CreateFile(
		g.packageType.fileName(), g.packageType.outputDir(outputDir), lang, false)
	if err != nil {
		return err
	}
	g.rootFile = rootFile
	if err = g.GenerateDocStringComment(g.rootFile); err != nil {
		return err
	}
	if err = g.GenerateNewline(g.rootFile, 2); err != nil {
		return err
	}
	// TODO: externs go here
	return nil
}

func (g *Generator) TeardownGenerator() error {
	defer g.rootFile.Close()
	return g.PostProcess(g.rootFile)
}

func (g *Generator) crateName() string {
	crateName := g.Frugal.Name

	if namespace := g.Frugal.Namespace(lang); namespace != nil {
		path := generator.GetPackageComponents(namespace.Value)
		crateName = strings.Join(path, "_")
	}

	return crateName
}

func (g *Generator) GetOutputDir(dir string) string {
	return filepath.Join(dir, g.crateName())
}

func (g *Generator) DefaultOutputDir() string {
	return defaultOutputDir
}

func (g *Generator) PostProcess(f *os.File) error {
	if err := f.Sync(); err != nil {
		return err
	}
	_, err := f.Seek(0, 0)
	if err != nil {
		return err
	}
	cmd := exec.Command("rustfmt")
	cmd.Stdin = f
	var out bytes.Buffer
	var e bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &e
	if err := cmd.Run(); err != nil {
		if strings.Contains(err.Error(), "exit status 2") {
			return errors.New(string(e.Bytes()))
		}
		return err
	}
	f.Seek(0, 0)
	f.Truncate(0)
	_, err = f.Write(out.Bytes())
	return err
}

func (g *Generator) GenerateDependencies(dir string) error {
	if !g.packageType.generateCargoTOML() {
		return nil
	}

	cargoFile, err := g.CreateFile("Cargo", dir, "toml", false)
	if err != nil {
		return err
	}

	cargoFile.WriteString(fmt.Sprintf(`[package]
name = %q
version = %q

[dependencies]`, g.crateName(), globals.Version))
	return cargoFile.Close()
}

func (g *Generator) GenerateFile(name, outputDir string, fileType generator.FileType) (*os.File, error) {
	switch fileType {
	case generator.CombinedServiceFile:
		return g.CreateFile(strings.ToLower(name)+serviceSuffix, g.packageType.outputDir(outputDir), lang, false)
	case generator.CombinedScopeFile:
		return g.CreateFile(strings.ToLower(name)+scopeSuffix, g.packageType.outputDir(outputDir), lang, false)
	default:
		return nil, fmt.Errorf("Bad file type for rust generator: %s", fileType)
	}
}

// GenerateDocStringComment generates the autogenerated notice.
func (g *Generator) GenerateDocStringComment(file *os.File) error {
	comment := fmt.Sprintf(
		`// Autogenerated by Frugal Compiler (%s)
// DO NOT EDIT UNLESS YOU ARE SURE THAT YOU KNOW WHAT YOU ARE DOING`,
		globals.Version)

	_, err := file.WriteString(comment)
	return err
}

func (g *Generator) GenerateServicePackage(_ *os.File, s *parser.Service) error {
	_, err := g.rootFile.WriteString(fmt.Sprintf("pub mod %s_service;\n", strings.ToLower(s.Name)))
	return err
}

func (g *Generator) GenerateScopePackage(_ *os.File, s *parser.Scope) error {
	_, err := g.rootFile.WriteString(fmt.Sprintf("pub mod %s_scope;\n", strings.ToLower(s.Name)))
	return err
}

func (g *Generator) generateRustLiteral(t *parser.Type, value interface{}) string {
	switch v := value.(type) {
	case string, parser.Identifier:
		return fmt.Sprintf("%q", v)
	case []parser.KeyValue:
		var buffer bytes.Buffer
		//for _, kv := range v {

		//}
		return buffer.String()
	default:
		return fmt.Sprintf("%v", v)
	}
}

func (g *Generator) writeDocComment(buffer bytes.Buffer, comments []string) {
	for _, comment := range comments {
		buffer.WriteString(fmt.Sprintf("/// %s\n", comment))
	}
}

func (g *Generator) GenerateConstantsContents(constants []*parser.Constant) error {
	var buffer bytes.Buffer
	for _, constant := range constants {
		g.writeDocComment(buffer, constant.Comment)
		// pub const NAME: TYPE = VALUE;
		// or
		// pub static NAME: TYPE = VALUE: Are statics only needed for containers?
		if !constant.Type.IsContainer() {
			t := constant.Type
			//if t == "string" {
			//	t = "&'static str"
			//} else if t == "binary" {
			//	t = "&'static [u8]"
			//}
			buffer.WriteString(fmt.Sprintf("pub const %s: %s = %v;\n\n", constant.Name, g.toRustType(t), g.generateRustLiteral(constant.Type, constant.Value)))
		}
	}
	_, err := g.rootFile.Write(buffer.Bytes())
	return err
}

// typeName takes a string and converts it to Upper Camelcase
func typeName(s string) string {
	if len(s) == 0 {
		return s
	}

	var buffer bytes.Buffer

	words := strings.Split(s, "_")

	for _, word := range words {
		w := []rune(word)
		w[0] = unicode.ToUpper(w[0])
		buffer.WriteString(string(w))
	}

	return buffer.String()
}

func (g *Generator) GenerateTypeDef(typedef *parser.TypeDef) error {
	var buffer bytes.Buffer
	g.writeDocComment(buffer, typedef.Comment)
	buffer.WriteString(fmt.Sprintf("pub type %s = %s;\n\n", typeName(typedef.Name), g.toRustType(typedef.Type)))
	_, err := g.rootFile.Write(buffer.Bytes())
	return err
}

func (g *Generator) GenerateEnum(enum *parser.Enum) error {
	var buffer bytes.Buffer
	g.writeDocComment(buffer, enum.Comment)
	eName := typeName(enum.Name)
	buffer.WriteString("#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]\n")
	buffer.WriteString(fmt.Sprintf("pub enum %s{\n", eName))
	for _, v := range enum.Values {
		g.writeDocComment(buffer, v.Comment)
		buffer.WriteString(fmt.Sprintf("%s = %v,\n", v.Name, v.Value))
	}
	buffer.WriteString(fmt.Sprintf("}\n\n"))
	_, err := g.rootFile.Write(buffer.Bytes())
	return err
}

func commaSpaceJoin(s []string) string {
	return strings.Join(s, ", ")
}

func angleBracket(s string) string {
	if s != "" {
		return fmt.Sprintf("<%s>", s)
	}
	return s
}

func where(s string) string {
	if s != "" {
		return fmt.Sprintf("where %s", s)
	}
	return s
}

func (g *Generator) GenerateStruct(s *parser.Struct) error {
	var buffer bytes.Buffer

	// write the struct def itself
	g.writeDocComment(buffer, s.Comment)
	sName := typeName(s.Name)
	buffer.WriteString("#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]\n")
	buffer.WriteString(fmt.Sprintf("pub struct %s {\n", sName))
	typeParams := make([]string, 0, len(s.Fields))
	args := make([]string, 0, len(s.Fields))
	whereClause := make([]string, 0, len(s.Fields))
	for i, f := range s.Fields {
		g.writeDocComment(buffer, f.Comment)
		t := g.toRustType(f.Type)
		if f.Modifier != parser.Required {
			t = fmt.Sprintf("Option<%s>", t)
		}
		buffer.WriteString(fmt.Sprintf("pub %s: %s,\n", f.Name, t))

		// the following are needed in the impl block
		fTypeParam := fmt.Sprintf("F%v", i)
		typeParams = append(typeParams, fTypeParam)
		args = append(args, fmt.Sprintf("%s: %s", f.Name, fTypeParam))
		whereClause = append(whereClause, fmt.Sprintf("%s: Into<%s>", fTypeParam, t))

	}
	buffer.WriteString("}\n\n")

	// now the impl block
	buffer.WriteString(fmt.Sprintf("impl %s {\n", sName))
	buffer.WriteString(fmt.Sprintf("pub fn new%s(%s) -> %s %s {\n", angleBracket(commaSpaceJoin(typeParams)), commaSpaceJoin(args), sName, where(commaSpaceJoin(whereClause))))
	buffer.WriteString(fmt.Sprintf("%s {\n", sName))
	for _, f := range s.Fields {
		buffer.WriteString(fmt.Sprintf("%s: %s.into(),\n", f.Name, f.Name))
	}
	buffer.WriteString("}\n")
	buffer.WriteString("}\n")
	buffer.WriteString("}\n\n")

	_, err := g.rootFile.Write(buffer.Bytes())
	return err
}

func (g *Generator) GenerateUnion(union *parser.Struct) error {
	return nil
}

func (g *Generator) GenerateException(exception *parser.Struct) error {
	// TODO: Implement the failure crate for these
	return g.GenerateStruct(exception)
}

func (g *Generator) GenerateTypesImports(file *os.File) error {
	return nil
}

func (g *Generator) GenerateServiceImports(file *os.File, s *parser.Service) error {
	// TODO: Handle other imports?
	_, err := file.WriteString("use super::*;\n")
	return err
}

func (g *Generator) GenerateScopeImports(file *os.File, s *parser.Scope) error { return nil }

func (g *Generator) GenerateConstants(file *os.File, name string) error { return nil }

func (g *Generator) GeneratePublisher(file *os.File, scope *parser.Scope) error { return nil }

func (g *Generator) GenerateSubscriber(file *os.File, scope *parser.Scope) error { return nil }

// methodName takes a methodname, typically in lowerCamelCase and converts it to snake_case
func methodName(s string) string {
	if len(s) == 0 {
		return s
	}

	runes := []rune(s)
	i := 1
	var buffer bytes.Buffer
	buffer.WriteRune(unicode.ToLower(runes[0]))
	addedUnderscore := false
	for i < len(runes) {
		if unicode.IsLower(runes[i]) {
			buffer.WriteRune(runes[i])
			if i+1 < len(runes) {
				if unicode.IsUpper(runes[i+1]) {
					buffer.WriteRune('_')
					addedUnderscore = true
				}
			}
			i++
			continue
		}

		if i+1 < len(runes) {
			if unicode.IsLower(runes[i+1]) && !addedUnderscore {
				buffer.WriteRune('_')
			}
		}
		addedUnderscore = false

		buffer.WriteRune(unicode.ToLower(runes[i]))
		i++
	}
	return buffer.String()
}

func (g *Generator) GenerateService(file *os.File, s *parser.Service) error {
	var buffer bytes.Buffer

	// write the service trait
	g.writeDocComment(buffer, s.Comment)
	extends := strings.Replace(s.Extends, ".", "::", -1)
	sName := typeName(s.Name)
	buffer.WriteString(fmt.Sprintf("pub trait F%s%s {\n", sName, extends))
	for _, method := range s.Methods {
		g.writeDocComment(buffer, method.Comment)

		args := make([]string, 0, len(method.Arguments))
		for _, f := range method.Arguments {
			t := g.toRustType(f.Type)
			if f.Modifier != parser.Required {
				t = fmt.Sprintf("Option<%s>", t)
			}
			args = append(args, fmt.Sprintf("%s: %s", f.Name, t))
		}

		buffer.WriteString(fmt.Sprintf("fn %s(ctx: FContext, %s) -> thrift::Result<%s>;\n", methodName(method.Name), commaSpaceJoin(args), g.toRustType(method.ReturnType)))
	}
	buffer.WriteString("}\n")

	// TODO: write the service client

	// TODO: write the service processor

	_, err := file.Write(buffer.Bytes())
	return err
}

func (g *Generator) toRustType(t *parser.Type) string {
	if t == nil {
		return "()"
	}
	switch t.Name {
	case "bool":
		return "bool"
	case "byte":
		return "u8"
	case "i8":
		return "i8"
	case "i16":
		return "i16"
	case "i32":
		return "i32"
	case "i64":
		return "i64"
	case "double":
		return "OrderedFloat<f64>"
	case "string":
		return "String"
	case "binary":
		return "Vec<u8>"
	case "list":
		return fmt.Sprintf("Vec<%s>", g.toRustType(t.ValueType))
	case "set":
		return fmt.Sprintf("BTreeSet<%s>", g.toRustType(t.ValueType))
	case "map":
		return fmt.Sprintf("BTreeMap<%s, %s>",
			g.toRustType(t.KeyType),
			g.toRustType(t.ValueType))
	default:
		// Custom type, either typedef or struct.
		// TODO: How to handle this type name?
		name := strings.Title(t.Name)
		if strings.Contains(name, ".") {
			name = strings.Replace(name, ".", "::", -1)
		}
		return name
	}
}
