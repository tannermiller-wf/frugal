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
// TODO: json serialization is a thing here, look at bringing in serde

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

func (g *Generator) UseVendor() bool {
	return false
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

	includedCrates := g.iterateIncludes("extern crate %s;", 1)
	_, err = g.rootFile.WriteString(fmt.Sprintf(
		`#![allow(deprecated)]
		 #![allow(unused_imports)]
		
		 %s
		 extern crate frugal;
		 extern crate thrift;
		 #[macro_use]
		 extern crate lazy_static;
		 #[macro_use]
		 extern crate log;
		 extern crate futures;
		 extern crate tower_service;
		 extern crate tower_web;
		
		 use std::collections::{BTreeMap, BTreeSet};

		`,
		strings.Join(includedCrates, "\n"),
	))
	return err
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

	return methodName(crateName)
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
		if strings.Contains(err.Error(), "exit status 1") {
			return errors.New(string(e.Bytes()))
		}
		return err
	}
	f.Seek(0, 0)
	f.Truncate(0)
	_, err = f.Write(out.Bytes())
	return err
}

func (g *Generator) iterateIncludes(format string, n int) []string {
	includedCrates := []string{}
	for _, include := range g.Frugal.Includes {
		namespace := g.Frugal.NamespaceForInclude(include.Name, lang)
		includeName := include.Name
		if namespace != nil {
			includeName = includeNameToReference(namespace.Value)
		}
		name := methodName(includeName)
		inputs := make([]interface{}, n)
		for i := 0; i < n; i++ {
			inputs[i] = name
		}
		includedCrates = append(includedCrates, fmt.Sprintf(format, inputs...))
	}
	return includedCrates
}

func (g *Generator) GenerateDependencies(dir string) error {
	if !g.packageType.generateCargoTOML() {
		return nil
	}

	cargoFile, err := g.CreateFile("Cargo", dir, "toml", false)
	if err != nil {
		return err
	}

	includedCrates := g.iterateIncludes(`%s = { path = "../%s" }`, 2)

	cargoFile.WriteString(fmt.Sprintf(`[package]
name = %q
version = %q

[dependencies]
thrift = { path = "../../../lib/rust/thrift" }
frugal = { path = "../../../lib/rust" }
lazy_static = "1.2"
log = "0.4"
tower-service = "0.1"
tower-web = "0.3"
futures = "0.1"
%s
`, g.crateName(), globals.Version, strings.Join(includedCrates, "\n")))
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

func includeNameToReference(includeName string) string {
	split := strings.FieldsFunc(includeName, func(r rune) bool {
		return r == '.' || r == '/'
	})
	return strings.Join(split, "_")
}

func (g *Generator) generateRustLiteral(t *parser.Type, value interface{}, optional bool) string {
	underlyingType := g.Frugal.UnderlyingType(t)
	var name string
	if identifier, ok := value.(parser.Identifier); ok {
		idCtx := g.Frugal.ContextFromIdentifier(identifier)
		switch idCtx.Type {
		case parser.LocalConstant:
			name = strings.ToUpper(idCtx.Constant.Name)
			if g.isBorrowed(underlyingType) {
				name = fmt.Sprintf("%s.clone()", name)
			}
		case parser.LocalEnum:
			name = fmt.Sprintf("%s::%s", typeName(idCtx.Enum.Name), title(strings.ToLower(idCtx.EnumValue.Name)))
		case parser.IncludeConstant:
			include := idCtx.Include.Name
			if namespace := g.Frugal.NamespaceForInclude(include, lang); namespace != nil {
				include = namespace.Value
			}
			name = fmt.Sprintf("%s::%s", includeNameToReference(include), strings.ToUpper(idCtx.Constant.Name))
		case parser.IncludeEnum:
			include := idCtx.Include.Name
			if namespace := g.Frugal.NamespaceForInclude(include, lang); namespace != nil {
				include = namespace.Value
			}
			name = fmt.Sprintf("%s::%s::%s", includeNameToReference(include), typeName(idCtx.Enum.Name), title(strings.ToLower(idCtx.EnumValue.Name)))
		default:
			panic(fmt.Sprintf("The Identifier %s has unexpected type %d", identifier, idCtx.Type))
		}
	} else {
		if underlyingType.IsPrimitive() || underlyingType.IsContainer() {
			switch underlyingType.Name {
			case "bool", "i8", "byte", "i16", "i32", "i64", "double":
				name = fmt.Sprintf("%v", value)
			case "string":
				name = fmt.Sprintf("%q.into()", value.(string))
			case "binary":
				name = fmt.Sprintf("b%q.to_vec()", value)
			case "list":
				var buffer bytes.Buffer
				buffer.WriteString("vec![\n")
				values := value.([]interface{})
				encodedValues := make([]string, 0, len(values))
				for _, v := range values {
					encodedValues = append(encodedValues, g.generateRustLiteral(underlyingType.ValueType, v, false))
				}
				buffer.WriteString(strings.Join(encodedValues, ", "))
				buffer.WriteString("]")
				name = buffer.String()
			case "set":
				var buffer bytes.Buffer
				buffer.WriteString(fmt.Sprintf("{\nlet mut s = BTreeSet::new();\n"))
				for _, v := range value.([]interface{}) {
					buffer.WriteString(fmt.Sprintf("s.insert(%s);\n", g.generateRustLiteral(underlyingType.ValueType, v, false)))
				}
				buffer.WriteString("s\n}")
				name = buffer.String()
			case "map":
				var buffer bytes.Buffer
				buffer.WriteString(fmt.Sprintf("{\nlet mut m = BTreeMap::new();\n"))
				for _, pair := range value.([]parser.KeyValue) {
					key := g.generateRustLiteral(underlyingType.KeyType, pair.Key, false)
					val := g.generateRustLiteral(underlyingType.ValueType, pair.Value, false)
					buffer.WriteString(fmt.Sprintf("m.insert(%s, %s);\n", key, val))
				}
				buffer.WriteString("m\n}")
				name = buffer.String()
			}
		} else if g.Frugal.IsEnum(underlyingType) {
			e := g.Frugal.FindEnum(underlyingType)
			if e == nil {
				panic("no enum for type " + underlyingType.Name)
			}
			vi, ok := value.(int64)
			if !ok {
				panic(fmt.Sprintf("Enum value %v is not an int64", value))
			}
			for _, ev := range e.Values {
				if ev.Value == int(vi) {
					name = fmt.Sprintf("%s::%s", g.toRustType(underlyingType, false), title(strings.ToLower(ev.Name)))
					break
				}
			}
			if name == "" {
				panic(fmt.Sprintf("no enum value %v for type %s", value, underlyingType.Name))
			}
		} else if g.Frugal.IsStruct(underlyingType) {
			s := g.Frugal.FindStruct(underlyingType)
			if s == nil {
				panic("no struct for type " + underlyingType.Name)
			}

			var buffer bytes.Buffer
			buffer.WriteString(fmt.Sprintf("%s{\n", g.toRustType(underlyingType, false)))

			for _, pair := range value.([]parser.KeyValue) {
				for _, field := range s.Fields {
					if pair.KeyToString() == field.Name {
						val := g.generateRustLiteral(field.Type, pair.Value, field.Modifier != parser.Required)
						buffer.WriteString(fmt.Sprintf("%s: %s,\n", methodName(pair.KeyToString()), val))
					}
				}
			}

			buffer.WriteString("}")
			name = buffer.String()
		}
		if name == "" {
			panic("no entry for type " + underlyingType.Name)
		}
	}

	if optional {
		name = fmt.Sprintf("Some(%s)", name)
	}
	return name
}

func (g *Generator) writeDocComment(buffer *bytes.Buffer, comments []string) {
	for _, comment := range comments {
		buffer.WriteString(fmt.Sprintf("/// %s\n", comment))
	}
}

// Currently only deprecated is supported, all others are ignored
func (g *Generator) writeAnnotations(buffer *bytes.Buffer, annotations parser.Annotations) {
	if note, ok := annotations.Deprecated(); ok {
		buffer.WriteString(fmt.Sprintf("#[deprecated(note = %q)]\n", note))
	}
}

func (g *Generator) GenerateConstantsContents(constants []*parser.Constant) error {
	var buffer bytes.Buffer
	for _, constant := range constants {
		g.writeDocComment(&buffer, constant.Comment)
		g.writeAnnotations(&buffer, constant.Annotations)
		// pub const NAME: TYPE = VALUE;
		// or
		// pub static NAME: TYPE = VALUE: Are statics only needed for containers?
		if constant.Type.IsContainer() || g.Frugal.IsStruct(constant.Type) || g.Frugal.IsEnum(constant.Type) || constant.Type.Name == "binary" || constant.Type.Name == "string" {
			buffer.WriteString(fmt.Sprintf("lazy_static! {\npub static ref %s: %s = %v;\n}\n\n", strings.ToUpper(constant.Name), g.toRustType(constant.Type, false), g.generateRustLiteral(constant.Type, constant.Value, false)))
		} else {
			buffer.WriteString(fmt.Sprintf("pub const %s: %s = %v;\n\n", strings.ToUpper(constant.Name), g.toRustType(constant.Type, false), g.generateRustLiteral(constant.Type, constant.Value, false)))
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
	g.writeDocComment(&buffer, typedef.Comment)
	g.writeAnnotations(&buffer, typedef.Annotations)
	buffer.WriteString(fmt.Sprintf("pub type %s = %s;\n\n", typeName(typedef.Name), g.toRustType(typedef.Type, false)))
	_, err := g.rootFile.Write(buffer.Bytes())
	return err
}

func (g *Generator) GenerateEnum(enum *parser.Enum) error {
	var buffer bytes.Buffer
	g.writeDocComment(&buffer, enum.Comment)
	g.writeAnnotations(&buffer, enum.Annotations)
	eName := typeName(enum.Name)
	buffer.WriteString("#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]\n")
	buffer.WriteString(fmt.Sprintf("pub enum %s{\n", eName))
	for _, v := range enum.Values {
		g.writeDocComment(&buffer, v.Comment)
		g.writeAnnotations(&buffer, v.Annotations)
		buffer.WriteString(fmt.Sprintf("%s = %v,\n", title(strings.ToLower(v.Name)), v.Value))
	}
	buffer.WriteString(fmt.Sprintf("}\n\n"))

	// impl block
	buffer.WriteString(fmt.Sprintf("impl %s {\n", eName))

	// from_i32 method
	buffer.WriteString(fmt.Sprintf(
		`pub fn from_i32(i: i32) -> thrift::Result<%s> {
			 match i {
				 `,
		eName))
	for _, v := range enum.Values {
		vName := title(strings.ToLower(v.Name))
		buffer.WriteString(fmt.Sprintf("i if %s::%s as i32 == i => Ok(%s::%s),\n", eName, vName, eName, vName))
	}
	buffer.WriteString(fmt.Sprintf(
		`		_ => Err(thrift::new_protocol_error(
					thrift::ProtocolErrorKind::InvalidData,
					format!("{} is not a valid integer value for %s", i),
				)),
			}
		}`,
		eName,
	))

	buffer.WriteString("}\n\n")

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
	_, err := g.rootFile.Write([]byte(g.generateStruct(s, func(s string) string { return s })))
	return err

}

func (g *Generator) generateStruct(s *parser.Struct, writeStructID func(string) string) string {
	var buffer bytes.Buffer

	// write the struct def itself
	g.writeDocComment(&buffer, s.Comment)
	g.writeAnnotations(&buffer, s.Annotations)
	sName := typeName(s.Name)
	deriveDefault := "Default, "
	for _, f := range s.Fields {
		if f.Default != nil {

			deriveDefault = ""
			break
		}
	}
	buffer.WriteString(fmt.Sprintf("#[derive(Clone, Debug, %sEq, Ord, PartialEq, PartialOrd)]\n", deriveDefault))
	buffer.WriteString(fmt.Sprintf("pub struct %s {\n", sName))
	constructorExpressions := make([]string, 0, len(s.Fields))
	for _, f := range s.Fields {
		g.writeDocComment(&buffer, f.Comment)
		g.writeAnnotations(&buffer, f.Annotations)
		t := g.toRustType(f.Type, f.Modifier != parser.Required)
		fieldName := methodName(f.Name)
		buffer.WriteString(fmt.Sprintf("pub %s: %s,\n", fieldName, t))

		// the following are needed in the impl block
		if deriveDefault == "" {
			if f.Default != nil {
				defaultValue := g.generateRustLiteral(f.Type, f.Default, false)
				if f.Modifier != parser.Required {
					defaultValue = fmt.Sprintf("Some(%s)", defaultValue)
				}
				constructorExpressions = append(constructorExpressions, fmt.Sprintf("%s: %s,", fieldName, defaultValue))
			} else {
				constructorExpressions = append(constructorExpressions, fmt.Sprintf("%s: Default::default(),", fieldName))
			}
		}
	}
	buffer.WriteString("}\n\n")

	// explicitly impl Default if defaults are set
	if deriveDefault == "" {
		buffer.WriteString(fmt.Sprintf(`impl Default for %s {
			fn default() -> %s {
				%s {
					%s
				}
			}
		}
		
		`, sName, sName, sName, strings.Join(constructorExpressions, "\n")))
	}

	// now the impl block
	buffer.WriteString(fmt.Sprintf("impl %s {\n", sName))

	// primary read method
	buffer.WriteString(
		`pub fn read<R, T>(&mut self, iprot: &mut T) -> thrift::Result<()>
		 where
			R: thrift::transport::TReadTransport,
		 	T: thrift::protocol::TInputProtocol<R>,
		 {
		 	iprot.read_struct_begin()?;
		 	loop {
				let field_id = iprot.read_field_begin()?;
				if field_id.field_type == thrift::protocol::TType::Stop {
					break;
				};
				match field_id.id {
					`,
	)
	for _, f := range s.Fields {
		buffer.WriteString(fmt.Sprintf("Some(%v) => self.read_field_%v(iprot)?,\n", f.ID, f.ID))
	}
	buffer.WriteString(
		`			_ => iprot.skip(field_id.field_type)?,
				};
				iprot.read_field_end()?;
			}
		 	iprot.read_struct_end()
	 	}
		
		`,
	)

	// field read methods
	for _, f := range s.Fields {
		buffer.WriteString(fmt.Sprintf(
			`fn read_field_%v<R, T>(&mut self, iprot: &mut T) -> thrift::Result<()>
    		 where
				 R: thrift::transport::TReadTransport,
    		     T: thrift::protocol::TInputProtocol<R>,
    		 {`,
			f.ID,
		))
		fieldName := methodName(f.Name)
		fType := g.Frugal.UnderlyingType(f.Type)
		g.generateFieldReadDefinition(&buffer, fieldName, fType)
		varExpr := fieldName
		if f.Modifier != parser.Required {
			varExpr = fmt.Sprintf("Some(%s)", varExpr)
		}
		buffer.WriteString(fmt.Sprintf(
			`	self.%s = %s;
				Ok(())
			}
			
			`,
			fieldName,
			varExpr,
		))
	}

	// primary write method
	buffer.WriteString(fmt.Sprintf(
		`pub fn write<W, T>(&self, oprot: &mut T) -> thrift::Result<()>
		 where
			W: thrift::transport::TWriteTransport,
		 	T: thrift::protocol::TOutputProtocol<W>,
		 {
			 oprot.write_struct_begin(&thrift::protocol::TStructIdentifier::new(%q))?;`,
		writeStructID(s.Name),
	))
	for _, f := range s.Fields {
		buffer.WriteString(fmt.Sprintf("self.write_field_%d(oprot)?;\n", f.ID))
	}
	buffer.WriteString(
		`	oprot.write_field_stop()?;
			oprot.write_struct_end()
	 	}
		
		`,
	)

	// field write methods
	for _, f := range s.Fields {
		buffer.WriteString(fmt.Sprintf(
			`fn write_field_%v<W, T>(&self, oprot: &mut T) -> thrift::Result<()>
    		 where
				 W: thrift::transport::TWriteTransport,
    		     T: thrift::protocol::TOutputProtocol<W>,
    		 {`,
			f.ID,
		))
		fieldName := methodName(f.Name)
		fType := g.Frugal.UnderlyingType(f.Type)
		if f.Modifier == parser.Required {
			borrow := ""
			if g.isBorrowed(fType) {
				borrow = "&"
			}
			buffer.WriteString(fmt.Sprintf("let %s = %sself.%s;\n", fieldName, borrow, fieldName))
		} else {
			maybeRefFieldName := fieldName
			if g.isBorrowed(fType) {
				maybeRefFieldName = fmt.Sprintf("ref %s", fieldName)
			}
			buffer.WriteString(fmt.Sprintf(
				`let %s = match self.%s {
					 Some(%s) => %s,
					 None => return Ok(()),
				 };
			`,
				fieldName, fieldName, maybeRefFieldName, fieldName,
			))
		}
		buffer.WriteString(fmt.Sprintf(
			`oprot.write_field_begin(&thrift::protocol::TFieldIdentifier::new(
				%q,
				thrift::protocol::TType::%s,
				%d,
			))?;
			`,
			f.Name, g.ttypeToEnum(fType), f.ID,
		))
		g.generateFieldWriteDefinition(&buffer, fieldName, fType, false)
		buffer.WriteString(
			`	oprot.write_field_end()
			}
			
			`,
		)
	}

	buffer.WriteString("}\n\n")

	return buffer.String()
}

func (g *Generator) isBorrowed(t *parser.Type) bool {
	if t.IsContainer() || g.Frugal.IsStruct(t) || g.Frugal.IsEnum(t) || t.Name == "binary" || t.Name == "string" {
		return true
	}
	return false
}

func (g *Generator) generateFieldReadDefinition(buffer *bytes.Buffer, fieldName string, fType *parser.Type) {
	if fType.IsPrimitive() {
		fTypeName := fType.Name
		if fTypeName == "binary" {
			fTypeName = "bytes"
		}
		buffer.WriteString(fmt.Sprintf("let %s = iprot.read_%s()?;\n", fieldName, fTypeName))
	} else if g.Frugal.IsEnum(fType) {
		eName := g.canonicalizeTypeName(fType)
		buffer.WriteString(fmt.Sprintf("let %s = iprot.read_i32().and_then(%s::from_i32)?;\n", fieldName, eName))
	} else if g.Frugal.IsUnion(fType) {
		//uName := g.canonicalizeTypeName(fType)
		buffer.WriteString(fmt.Sprintf(
			`let mut %s = %s;
			 %s.read(iprot)?;
			`,
			fieldName, g.zeroValue(fType), fieldName,
		))
	} else if fType.IsCustom() {
		tName := g.canonicalizeTypeName(fType)
		buffer.WriteString(fmt.Sprintf(
			`let mut %s = %s::default();
			 %s.read(iprot)?;
			`, fieldName, tName, fieldName))
	} else if fType.Name == "list" {
		buffer.WriteString(fmt.Sprintf(
			`let list_id = iprot.read_list_begin()?;
			 let mut %s = Vec::with_capacity(list_id.size as usize);
			 for _ in 0..list_id.size {
				 `,
			fieldName,
		))
		itemName := fmt.Sprintf("%s_item", fieldName)
		vType := g.Frugal.UnderlyingType(fType.ValueType)
		g.generateFieldReadDefinition(buffer, itemName, vType)
		buffer.WriteString(fmt.Sprintf(
			`	%s.push(%s);
			}
			 iprot.read_list_end()?;
			 `, fieldName, itemName,
		))
	} else if fType.Name == "map" {
		buffer.WriteString(fmt.Sprintf(
			`let map_id = iprot.read_map_begin()?;
			 let mut %s = BTreeMap::new();
			 for _ in 0..map_id.size {`,
			fieldName,
		))
		keyName := fmt.Sprintf("%s_key", fieldName)
		keyType := g.Frugal.UnderlyingType(fType.KeyType)
		g.generateFieldReadDefinition(buffer, keyName, keyType)
		valueName := fmt.Sprintf("%s_value", fieldName)
		valueType := g.Frugal.UnderlyingType(fType.ValueType)
		g.generateFieldReadDefinition(buffer, valueName, valueType)
		buffer.WriteString(fmt.Sprintf("%s.insert(%s, %s);\n", fieldName, keyName, valueName))
		buffer.WriteString(
			`}
			 iprot.read_map_end()?;
		`)
	} else if fType.Name == "set" {
		buffer.WriteString(fmt.Sprintf(
			`let set_id = iprot.read_set_begin()?;
			 let mut %s = BTreeSet::new();
			 for _ in 0..set_id.size {`,
			fieldName,
		))
		itemName := fmt.Sprintf("%s_item", fieldName)
		itemType := g.Frugal.UnderlyingType(fType.ValueType)
		g.generateFieldReadDefinition(buffer, itemName, itemType)
		buffer.WriteString(fmt.Sprintf("%s.insert(%s);\n", fieldName, itemName))
		buffer.WriteString(
			`}
			 iprot.read_set_end()?;
		`)
	}
}

func (g *Generator) ttypeToEnum(t *parser.Type) string {
	switch t.Name {
	case "i8":
		return "I08"
	case "bool", "i16", "i32", "i64", "double", "string", "list", "set", "map":
		return title(t.Name)
	case "binary":
		return "String"
	}

	if g.Frugal.IsEnum(t) {
		return "I32"
	}

	return "Struct"
}

func (g *Generator) generateFieldWriteDefinition(buffer *bytes.Buffer, fieldName string, fType *parser.Type, borrowed bool) {
	if fType.IsPrimitive() {
		varName := fieldName
		fTypeName := fType.Name
		if borrowed && fTypeName != "binary" && fTypeName != "string" {
			varName = fmt.Sprintf("*%s", varName)
		}
		if fTypeName == "binary" {
			fTypeName = "bytes"
		}
		buffer.WriteString(fmt.Sprintf("oprot.write_%s(%s)?;\n", fTypeName, varName))
	} else if g.Frugal.IsEnum(fType) {
		buffer.WriteString(fmt.Sprintf("oprot.write_i32(%s.clone() as i32)?;\n", fieldName))
	} else if fType.IsCustom() {
		buffer.WriteString(fmt.Sprintf("%s.write(oprot)?;\n", fieldName))
	} else if fType.Name == "list" {
		itemName := fmt.Sprintf("%s_item", fieldName)
		vType := g.Frugal.UnderlyingType(fType.ValueType)
		buffer.WriteString(fmt.Sprintf(
			`oprot.write_list_begin(&thrift::protocol::TListIdentifier::new(
				thrift::protocol::TType::%s,
				%s.len() as i32,
			 ))?;
			 for %s in %s {
			 `,
			g.ttypeToEnum(vType), fieldName, itemName, fieldName,
		))
		g.generateFieldWriteDefinition(buffer, itemName, vType, true)
		buffer.WriteString(
			`}
			 oprot.write_list_end()?;
			 `)
	} else if fType.Name == "set" {
		itemName := fmt.Sprintf("%s_item", fieldName)
		vType := g.Frugal.UnderlyingType(fType.ValueType)
		buffer.WriteString(fmt.Sprintf(
			`oprot.write_set_begin(&thrift::protocol::TSetIdentifier::new(
				thrift::protocol::TType::%s,
				%s.len() as i32,
			 ))?;
			 for %s in %s {
			 `,
			g.ttypeToEnum(vType), fieldName, itemName, fieldName,
		))
		g.generateFieldWriteDefinition(buffer, itemName, vType, true)
		buffer.WriteString(
			`}
			 oprot.write_set_end()?;
			 `)
	} else if fType.Name == "map" {
		kType := g.Frugal.UnderlyingType(fType.KeyType)
		keyname := fmt.Sprintf("%s_key", fieldName)
		valuename := fmt.Sprintf("%s_value", fieldName)
		vType := g.Frugal.UnderlyingType(fType.ValueType)
		buffer.WriteString(fmt.Sprintf(
			`oprot.write_map_begin(&thrift::protocol::TMapIdentifier::new(
				thrift::protocol::TType::%s,
				thrift::protocol::TType::%s,
				%s.len() as i32,
			 ))?;
			 for (%s, %s) in %s {
			 `,
			g.ttypeToEnum(kType), g.ttypeToEnum(vType), fieldName, keyname, valuename, fieldName,
		))
		g.generateFieldWriteDefinition(buffer, keyname, kType, true)
		g.generateFieldWriteDefinition(buffer, valuename, vType, true)
		buffer.WriteString(
			`}
        	 oprot.write_map_end()?;
			 `,
		)
	}
}

// zeroValue provides a value to be used in constructors where its a required field, but no default value is provided.
func (g *Generator) zeroValue(t *parser.Type) string {
	switch t.Name {
	case "bool":
		return "false"
	case "i8", "byte", "i16", "i32", "i64":
		return "0"
	case "double":
		return "0.0"
	case "string":
		return `""`
	case "binary", "list":
		return "Vec::new()"
	case "set":
		return "BTreeSet::new()"
	case "map":
		return "BTreeMap::new()"
	}

	if g.Frugal.IsUnion(t) {
		u := g.Frugal.FindUnion(t)
		uv := u.Fields[0]
		uvt := g.Frugal.UnderlyingType(uv.Type)
		return fmt.Sprintf("%s::%s(%s)", g.toRustType(t, false), typeName(uv.Name), g.zeroValue(uvt))
	} else if g.Frugal.IsStruct(t) {
		return fmt.Sprintf("%s::new()", typeName(t.Name))
	} else if g.Frugal.IsEnum(t) {
		e := g.Frugal.FindEnum(t)
		ev := e.Values[0]
		return fmt.Sprintf("%s::%s", g.toRustType(t, false), title(strings.ToLower(ev.Name)))
	}

	panic(fmt.Sprintf("cannot generate zero value for unrecognized type: %s", t.Name))
}

func (g *Generator) GenerateUnion(union *parser.Struct) error {
	var buffer bytes.Buffer
	g.writeDocComment(&buffer, union.Comment)
	g.writeAnnotations(&buffer, union.Annotations)
	uName := typeName(union.Name)
	buffer.WriteString("#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]\n")
	buffer.WriteString(fmt.Sprintf("pub enum %s{\n", uName))
	for _, f := range union.Fields {
		g.writeDocComment(&buffer, f.Comment)
		g.writeAnnotations(&buffer, f.Annotations)
		buffer.WriteString(fmt.Sprintf("%s(%s),\n", typeName(f.Name), g.toRustType(f.Type, false)))
	}
	buffer.WriteString(fmt.Sprintf("}\n\n"))

	// impl block
	buffer.WriteString(fmt.Sprintf("impl %s {\n", uName))

	// read methods
	buffer.WriteString(fmt.Sprintf(
		`pub fn read<R, T>(&mut self, iprot: &mut T) -> thrift::Result<()>
    	 where
		 	 R: thrift::transport::TReadTransport,
    	     T: thrift::protocol::TInputProtocol<R>,
    	 {
    	     iprot.read_struct_begin()?;
    	     let mut is_set = false;
    	     loop {
    	         let field_id = iprot.read_field_begin()?;
    	         if field_id.field_type == thrift::protocol::TType::Stop {
    	             break;
    	         };
    	         if is_set {
    	             return Err(thrift::new_protocol_error(
    	                 thrift::ProtocolErrorKind::InvalidData,
    	                 "%s read union: exactly one field must be set.",
    	             ));
    	         };
    	         match field_id.id {`,
		uName,
	))
	for _, f := range union.Fields {
		buffer.WriteString(fmt.Sprintf(
			`Some(%d) => {
				self.read_field_%d(iprot)?;
				is_set = true;
			}
			`,
			f.ID, f.ID,
		))
	}
	buffer.WriteString(fmt.Sprintf(
		`	 		_ => iprot.skip(field_id.field_type)?,
				};
				iprot.read_field_end()?;
			 }
			 iprot.read_struct_end()?;
			 if !is_set {
				 return Err(thrift::new_protocol_error(
					 thrift::ProtocolErrorKind::InvalidData,
					 "no field for union was sent",
				 ));
			 };
			 Ok(())
		 }
		 
		 `))

	// field read methods
	for _, f := range union.Fields {
		buffer.WriteString(fmt.Sprintf(
			`fn read_field_%d<R, T>(&mut self, iprot: &mut T) -> thrift::Result<()>
    	 	 where
			 	 R: thrift::transport::TReadTransport,
    	 	     T: thrift::protocol::TInputProtocol<R>,
    	 	 {
			 `,
			f.ID,
		))
		variant := typeName(f.Name)
		fieldName := methodName(f.Name)
		fType := g.Frugal.UnderlyingType(f.Type)
		g.generateFieldReadDefinition(&buffer, fieldName, fType)
		buffer.WriteString(fmt.Sprintf(
			`	*self = %s::%s(%s);
				Ok(())
			}

			`,
			uName, variant, fieldName,
		))

	}

	// write method
	buffer.WriteString(fmt.Sprintf(
		`pub fn write<W, T>(&self, oprot: &mut T) -> thrift::Result<()>
		 where
			W: thrift::transport::TWriteTransport,
		 	T: thrift::protocol::TOutputProtocol<W>,
		 {
			 oprot.write_struct_begin(&thrift::protocol::TStructIdentifier::new(%q))?;
			 match self {
			`,
		union.Name,
	))
	for _, f := range union.Fields {
		variant := typeName(f.Name)
		fieldName := methodName(f.Name)
		buffer.WriteString(fmt.Sprintf("%s::%s(%s) => {\n", uName, variant, fieldName))
		fType := g.Frugal.UnderlyingType(f.Type)
		g.generateFieldWriteDefinition(&buffer, fieldName, fType, true)
		buffer.WriteString("}")
	}
	buffer.WriteString(
		`	};
			oprot.write_field_stop()?;
			oprot.write_struct_end()
	 	}
		
		`,
	)

	buffer.WriteString("}\n\n")

	_, err := g.rootFile.Write(buffer.Bytes())
	return err
}

func (g *Generator) GenerateException(exception *parser.Struct) error {
	if err := g.GenerateStruct(exception); err != nil {
		return err
	}

	tName := typeName(exception.Name)

	var buffer bytes.Buffer
	buffer.WriteString(fmt.Sprintf("impl std::error::Error for %s {}\n\n", tName))

	buffer.WriteString(fmt.Sprintf("impl std::fmt::Display for %s {\n", tName))
	buffer.WriteString("fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {\nwrite!(f, \"{:?}\", self)\n}\n}\n\n")

	_, err := g.rootFile.Write(buffer.Bytes())
	return err
}

func (g *Generator) GenerateTypesImports(file *os.File) error {
	return nil
}

func (g *Generator) GenerateServiceImports(file *os.File, s *parser.Service) error {
	includedCrates := g.iterateIncludes("use %s;", 1)
	_, err := file.WriteString(fmt.Sprintf(
		`#![allow(unused_variables)]

		 use std::collections::BTreeMap;
		 use std::error::Error;
		 
		 use futures::future::{self, FutureResult};
		 use futures::{Async, Future, Poll};
		 use thrift;
		 use thrift::protocol::{TInputProtocol, TOutputProtocol};
		 use tower_service::Service;
		 use tower_web::middleware::{self, Middleware};
		 use tower_web::util::Chain;
		 
		 use frugal::buffer::FMemoryOutputBuffer;
		 use frugal::context::{FContext, OP_ID_HEADER};
		 use frugal::errors;
		 use frugal::processor::FProcessor;
		 use frugal::protocol::{
		     FInputProtocol, FInputProtocolFactory, FOutputProtocol, FOutputProtocolFactory,
		 };
		 use frugal::provider::FServiceProvider;
		 use frugal::transport::FTransport;

		 use super::*;
		 %s
		
		`, strings.Join(includedCrates, "\n"),
	))
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
			if unicode.IsLower(runes[i+1]) && !addedUnderscore && runes[i] != '_' {
				buffer.WriteRune('_')
			}
		}
		addedUnderscore = false

		buffer.WriteRune(unicode.ToLower(runes[i]))
		i++
	}
	return buffer.String()
}

func (g *Generator) generateMethodArguments(arguments []*parser.Field) []string {
	args := make([]string, 0, len(arguments))
	for _, f := range arguments {
		t := g.toRustType(f.Type, f.Modifier != parser.Required)
		args = append(args, fmt.Sprintf("%s: %s", methodName(f.Name), t))
	}
	return args
}

func (g *Generator) generateMethodSignature(method *parser.Method) string {
	return fmt.Sprintf("fn %s(&mut self, ctx: &FContext, %s) -> thrift::Result<%s>\n",
		methodName(method.Name), commaSpaceJoin(g.generateMethodArguments(method.Arguments)), g.toRustType(method.ReturnType, false))
}

func (g *Generator) implServiceForClient(implSvcName string, modName string, s *parser.Service, paramNames []string, svcBounds string) string {
	sName := typeName(s.Name)
	var buffer bytes.Buffer
	pNames := strings.Join(paramNames, ",")
	buffer.WriteString(fmt.Sprintf(`
				impl<%s> %sF%s for F%sClient<%s>
				where
					%s
				{
				`, pNames, modName, sName, implSvcName, pNames, svcBounds))
	for _, method := range s.Methods {
		mName := typeName(method.Name)
		args := make([]string, 0, len(method.Arguments))
		for _, f := range method.Arguments {
			args = append(args, methodName(f.Name))
		}
		buffer.WriteString(fmt.Sprintf("%s {\n", g.generateMethodSignature(method)))
		if modName == "" {
			buffer.WriteString(fmt.Sprintf(`let args = F%s%sArgs { %s };
						let request = F%sRequest::new(ctx.clone(), F%sMethod::%s(args));
						`,
				sName, mName, commaSpaceJoin(args),
				implSvcName, sName, mName))
			buffer.WriteString(fmt.Sprintf(`match self.service.call(request).wait()? {
							F%sResponse::%s(result) => `, sName, mName))
			if method.ReturnType == nil {
				buffer.WriteString("Ok(()),\n")
			} else {
				buffer.WriteString("{\n")
				for _, exc := range method.Exceptions {
					eName := methodName(exc.Name)
					buffer.WriteString(fmt.Sprintf(`if let Some(%s) = result.%s {
						return Err(thrift::Error::User(Box::new(%s)));
					};
					`, eName, eName, eName))
				}
				buffer.WriteString(fmt.Sprintf(`if let Some(success) = result.success {
						return Ok(success);
					};
					Err(thrift::Error::Application(thrift::ApplicationError::new(thrift::ApplicationErrorKind::MissingResult, "result was not returned for \"%s\"")))
				}`, mName))
			}
			if len(s.Methods) > 1 {
				buffer.WriteString(fmt.Sprintf(`_ => panic!("F%sClient::%s() received an incorrect response"),
					`, implSvcName, methodName(method.Name)))
			}
			// TODO: In the go code, there is a bunch of handling for catching payload too large exceptions
			buffer.WriteString("}\n")
		} else {
			buffer.WriteString(fmt.Sprintf("self.%s_client.%s(ctx, %s)", methodName(s.Name), methodName(method.Name), strings.Join(g.generateMethodArguments(method.Arguments), ",")))
		}
		buffer.WriteString("}\n\n")
	}
	buffer.WriteString("}\n\n")
	return buffer.String()
}

func (g *Generator) clientServiceTraits(s *parser.Service) ([]string, string) {
	svcBounds := g.clientServiceTraitsInner(s, "")
	paramNames := make([]string, 0, len(svcBounds))
	whereClause := make([]string, 0, len(svcBounds))
	for i, svcBound := range svcBounds {
		next := fmt.Sprintf("S%v", i)
		paramNames = append(paramNames, next)
		whereClause = append(whereClause, fmt.Sprintf("%s: %s", next, svcBound))
	}
	return paramNames, strings.Join(whereClause, ",\n")
}

func (g *Generator) clientServiceTraitsInner(s *parser.Service, include string) []string {
	sName := typeName(s.Name)
	svcBound := fmt.Sprintf(
		"Service<Request = %sF%sRequest, Response = %sF%sResponse, Error = thrift::Error>",
		include, sName, include, sName)
	if svc := s.ExtendsServiceStruct(); svc != nil {
		svcInclude := s.ExtendsInclude()
		if svcInclude != "" {
			if namespace := g.Frugal.NamespaceForInclude(svcInclude, lang); namespace != nil {
				svcInclude = includeNameToReference(namespace.Value)
			}
			svcInclude = fmt.Sprintf("%s::%s_service::", svcInclude, strings.ToLower(svc.Name))
		}
		return append(g.clientServiceTraitsInner(svc, svcInclude), svcBound)
	}
	return []string{svcBound}
}

func (g *Generator) processorUserErrorHandling(sName string, method *parser.Method) string {
	return g.processorUserErrorHandlingInner(sName, "user_err", 0, method)
}

func (g *Generator) processorUserErrorHandlingInner(sName string, subject string, index int, method *parser.Method) string {
	var buffer bytes.Buffer
	mName := typeName(method.Name)
	eName := methodName(method.Exceptions[index].Name)
	tName := method.Exceptions[index].Type.Name
	splitExtends := strings.Split(tName, ".")
	if len(splitExtends) > 1 {
		localModName := splitExtends[0]
		if namespace := g.Frugal.NamespaceForInclude(splitExtends[0], lang); namespace != nil {
			localModName = namespace.Value
		}
		tName = fmt.Sprintf("%s::%s", includeNameToReference(localModName), typeName(splitExtends[1]))
	} else {
		tName = typeName(tName)
	}
	buffer.WriteString(fmt.Sprintf(`%s
			.downcast::<%s>()
			.map(|%s| {
				F%sResponse::%s(F%s%sResult {`,
		subject, tName, eName, sName, mName, sName, mName))
	if method.ReturnType != nil {
		buffer.WriteString("success: None,\n")
	}
	buffer.WriteString(fmt.Sprintf("%s: Some(*%s),\n", eName, eName))
	for i, exc := range method.Exceptions {
		if i != index {
			buffer.WriteString(fmt.Sprintf("%s: None,\n", methodName(exc.Name)))
		}
	}
	buffer.WriteString("})\n})\n")
	if len(method.Exceptions) > index+1 {
		subj := fmt.Sprintf("not_%s", eName)
		buffer.WriteString(fmt.Sprintf(`.or_else(|%s| {
			%s
		})`, subj, g.processorUserErrorHandlingInner(sName, subj, index+1, method)))
	}
	return buffer.String()
}

func (g *Generator) GenerateService(file *os.File, s *parser.Service) error {
	var buffer bytes.Buffer

	// write the service trait
	g.writeDocComment(&buffer, s.Comment)
	g.writeAnnotations(&buffer, s.Annotations)
	sName := typeName(s.Name)
	extends := ""
	if s.Extends != "" {
		svcName := s.Extends
		modName := ""
		splitExtends := strings.Split(s.Extends, ".")
		if len(splitExtends) > 1 {
			svcName = splitExtends[1]
			localModName := splitExtends[0]
			if namespace := g.Frugal.NamespaceForInclude(splitExtends[0], lang); namespace != nil {
				localModName = namespace.Value
			}
			modName = fmt.Sprintf("%s::", includeNameToReference(localModName))
		}
		extends = fmt.Sprintf(": %s%s_service::F%s ", modName, strings.ToLower(svcName), typeName(svcName))
	}
	buffer.WriteString(fmt.Sprintf("pub trait F%s%s {\n", sName, extends))
	for _, method := range s.Methods {
		g.writeDocComment(&buffer, method.Comment)
		g.writeAnnotations(&buffer, method.Annotations)
		buffer.WriteString(fmt.Sprintf("%s;\n\n", g.generateMethodSignature(method)))
	}
	buffer.WriteString("}\n\n")

	// write the arg and result types that get serialized
	for _, method := range s.Methods {
		buffer.WriteString(g.generateStruct(methodToArgsStruct(sName, method), func(string) string { return fmt.Sprintf("%s_args", method.Name) }))
		buffer.WriteString(g.generateStruct(methodToResultStruct(sName, method), func(string) string { return fmt.Sprintf("%s_result", method.Name) }))
	}

	// write the other internal helper types
	buffer.WriteString(fmt.Sprintf("pub enum F%sMethod {\n", sName))
	for _, method := range s.Methods {
		mName := typeName(method.Name)
		buffer.WriteString(fmt.Sprintf("%s(F%s%sArgs),\n", mName, sName, mName))
	}
	buffer.WriteString(fmt.Sprintf(`}
	
		impl F%sMethod {
			fn name(&self) -> &'static str {
				match *self {
					`, sName))
	for _, method := range s.Methods {
		buffer.WriteString(fmt.Sprintf("F%sMethod::%s(_) => %q,\n", sName, typeName(method.Name), method.Name))
	}
	buffer.WriteString("}\n}\n}\n\n")

	buffer.WriteString(fmt.Sprintf(`pub struct F%sRequest {
			ctx: FContext,
			method: F%sMethod,
		}
		
		impl F%sRequest {
			pub fn new(ctx: FContext, method: F%sMethod) -> F%sRequest {
				F%sRequest { ctx, method }
			}
		}

		impl frugal::service::Request for F%sRequest {
			fn context(&mut self) -> &mut FContext {
				&mut self.ctx
			}

			fn method_name(&self) -> &'static str {
				self.method.name()
			}
		}
		
		pub enum F%sResponse {
			`, sName, sName, sName, sName, sName, sName, sName, sName))
	for _, method := range s.Methods {
		mName := typeName(method.Name)
		buffer.WriteString(fmt.Sprintf("%s(F%s%sResult),\n", mName, sName, mName))
	}
	buffer.WriteString("}\n\n")

	// write the service client
	paramNames, svcBounds := g.clientServiceTraits(s)
	extendServiceField := ""
	extendServiceFieldInst := ""
	extendsClientService := ""
	extendsModName := ""
	if s.Extends != "" {
		svcName := s.Extends
		splitExtends := strings.Split(s.Extends, ".")
		if len(splitExtends) > 1 {
			svcName = splitExtends[1]
			localModName := splitExtends[0]
			if namespace := g.Frugal.NamespaceForInclude(splitExtends[0], lang); namespace != nil {
				localModName = namespace.Value
			}
			extendsModName = fmt.Sprintf("%s::%s_service::", includeNameToReference(localModName), strings.ToLower(splitExtends[1]))
		}
		tName := typeName(svcName)
		mName := methodName(svcName)
		extendServiceField = fmt.Sprintf("%s_client: %sF%sClient<%s>,", mName, extendsModName, tName, "S0")
		extendServiceFieldInst = fmt.Sprintf("%s_client: %sF%sClient::new(provider.clone()),", mName, extendsModName, tName)
		extendsClientService = fmt.Sprintf("%sF%sClientService<T>, ", extendsModName, tName)
	}
	buffer.WriteString(fmt.Sprintf(`pub struct F%sClient<%s>
				where
					%s
				{
					%s
					service: S%v,
				}

				impl<T> F%sClient<%sF%sClientService<T>>
				where
					T: FTransport,
				{
					pub fn new(provider: FServiceProvider<T>) -> F%sClient<%sF%sClientService<T>> {
						F%sClient {
							%s
							service: F%sClientService {
								transport: provider.transport,
								input_protocol_factory: provider.input_protocol_factory,
								output_protocol_factory: provider.output_protocol_factory,
							}
						}
					}
				}
				`,
		sName, strings.Join(paramNames, ","), svcBounds, extendServiceField, len(paramNames)-1,
		sName, extendsClientService, sName, sName, extendsClientService, sName, sName, extendServiceFieldInst, sName))
	if svc := s.ExtendsServiceStruct(); svc != nil {
		buffer.WriteString(g.implServiceForClient(sName, extendsModName, svc, paramNames, svcBounds))
	}
	buffer.WriteString(g.implServiceForClient(sName, "", s, paramNames, svcBounds))
	buffer.WriteString(fmt.Sprintf(`

			pub struct F%sClientService<T>
			where
				T: FTransport,
			{
				transport: T,
		    	input_protocol_factory: FInputProtocolFactory,
		    	output_protocol_factory: FOutputProtocolFactory,
			}

			impl<T> F%sClientService<T>
			where
			    T: FTransport,
			{
			    fn call_delegate(&mut self, req: F%sRequest) -> Result<F%sResponse, thrift::Error> {
					enum ResultSignifier {
						`, sName, sName, sName, sName))
	for _, method := range s.Methods {
		buffer.WriteString(fmt.Sprintf("%s,\n", typeName(method.Name)))
	}
	buffer.WriteString(fmt.Sprintf(`
					};
					let F%sRequest { mut ctx, method } = req;
					let method_name = method.name();
					let mut buffer = FMemoryOutputBuffer::new(0);
					let signifier = {
						let mut oprot = self.output_protocol_factory.get_protocol(&mut buffer);
						oprot.write_request_header(&ctx)?;
						let mut oproxy = oprot.t_protocol_proxy();
						let signifier = match method {

			`, sName))
	for _, method := range s.Methods {
		mName := typeName(method.Name)
		buffer.WriteString(fmt.Sprintf(`F%sMethod::%s(args) => {
				oproxy.write_message_begin(&thrift::protocol::TMessageIdentifier::new(%q, thrift::protocol::TMessageType::Call, 0))?;
				let args = F%s%sArgs {
					`, sName, mName, method.Name, sName, mName))
		for _, arg := range method.Arguments {
			aName := methodName(arg.Name)
			buffer.WriteString(fmt.Sprintf("%s: args.%s,\n", aName, aName))
		}
		buffer.WriteString(fmt.Sprintf(`};
				args.write(&mut oproxy)?;
				ResultSignifier::%s
			}
			`, mName))
	}
	buffer.WriteString(`};
			oproxy.write_message_end()?;
			oproxy.flush()?;
			signifier
		};
		let mut result_transport = self.transport.request(&ctx, buffer.bytes())?;
		{
			let mut iprot = self
				.input_protocol_factory
				.get_protocol(&mut result_transport);
			iprot.read_response_header(&mut ctx)?;
			let mut iproxy = iprot.t_protocol_proxy();
			let msg_id = iproxy.read_message_begin()?;
			if msg_id.name != method_name {
				return Err(thrift::new_application_error(
					thrift::ApplicationErrorKind::WrongMethodName,
					format!("{} failed: wrong method name", method_name),
				));
			}
			match msg_id.message_type {
				thrift::protocol::TMessageType::Exception => {
					let err = thrift::Error::Application(
						thrift::Error::read_application_error_from_in_protocol(&mut iproxy)?,
					);
					iproxy.read_message_end()?;
					if frugal::errors::is_too_large_error(&err) {
						Err(thrift::new_transport_error(
							thrift::TransportErrorKind::SizeLimit,
							err.to_string(),
						))
					} else {
						Err(err)
					}
				}
				thrift::protocol::TMessageType::Reply => match signifier {
									`)
	for _, method := range s.Methods {
		mName := typeName(method.Name)
		buffer.WriteString(fmt.Sprintf(`
						ResultSignifier::%s => {
							let mut result = F%s%sResult::default();
							result.read(&mut iproxy)?;
							iproxy.read_message_end()?;
							Ok(F%sResponse::%s(result))
						}`, mName, sName, mName, sName, mName))
	}
	buffer.WriteString(fmt.Sprintf(`
				},
				_ => Err(thrift::new_application_error(
					thrift::ApplicationErrorKind::InvalidMessageType,
					format!("{} failed: invalid message type", method_name),
					)),
				}
			}
		}
	}

	impl<T> Service for F%sClientService<T>
	where
		T: FTransport,
	{
		type Request = F%sRequest;
		type Response = F%sResponse;
		type Error = thrift::Error;
		type Future = FutureResult<Self::Response, Self::Error>;

		fn poll_ready(&mut self) -> Poll<(), thrift::Error> {
			Ok(Async::Ready(()))
		}

		fn call(&mut self, req: Self::Request) -> Self::Future {
			self.call_delegate(req).into()
		}
	}
	
	`, sName, sName, sName))

	// write the service processor
	buffer.WriteString(fmt.Sprintf(`#[derive(Clone)]
	pub struct F%sProcessor<S>
	where
		S: Service<Request = F%sRequest, Response = F%sResponse, Error = thrift::Error>,
	{
		service: S,
	}
	
	pub struct F%sProcessorBuilder<F, M>
	where
		F: F%s,
	{
		handler: F,
		middleware: M,
	}
	
	impl<F> F%sProcessorBuilder<F, middleware::Identity>
	where
		F: F%s,
	{
		pub fn new(handler: F) -> Self {
			F%sProcessorBuilder {
				handler,
				middleware: middleware::Identity::new(),
			}
		}
	}
	
	impl<F, M> F%sProcessorBuilder<F, M>
	where
		F: F%s + Clone,
	{
    	pub fn middleware<U>(
    	    self,
    	    middleware: U,
    	) -> F%sProcessorBuilder<F, <M as Chain<U>>::Output>
    	where
    	    M: Chain<U>,
    	{
    	    F%sProcessorBuilder {
    	        handler: self.handler,
    	        middleware: self.middleware.chain(middleware),
    	    }
    	}

    	pub fn build(self) -> F%sProcessor<M::Service>
    	where
    	    M: Middleware<
    	        F%sProcessorService<F>,
    	        Request = F%sRequest,
    	        Response = F%sResponse,
    	        Error = thrift::Error,
    	    >,
    	{
    	    F%sProcessor {
    	        service: self.middleware.wrap(F%sProcessorService(self.handler)),
    	    }
    	}
	}

	#[derive(Clone)]
	pub struct F%sProcessorService<F: F%s + Clone>(F);
	
	impl<F> Service for F%sProcessorService<F>
	where
	    F: F%s + Clone,
	{
	    type Request = F%sRequest;
	    type Response = F%sResponse;
	    type Error = thrift::Error;
	    type Future = FutureResult<Self::Response, Self::Error>;
	
	    fn poll_ready(&mut self) -> Poll<(), thrift::Error> {
	        Ok(Async::Ready(()))
	    }
	
	    fn call(&mut self, req: F%sRequest) -> FutureResult<F%sResponse, thrift::Error> {
	        let result = match req.method {
				`, sName, sName, sName, sName, sName, sName, sName, sName, sName, sName, sName,
		sName, sName, sName, sName, sName, sName, sName, sName, sName, sName, sName, sName,
		sName, sName, sName))
	for _, method := range s.Methods {
		mName := typeName(method.Name)
		argSlice := make([]string, 0, len(method.Arguments))
		for _, arg := range method.Arguments {
			argSlice = append(argSlice, fmt.Sprintf("args.%s", methodName(arg.Name)))
		}
		args := commaSpaceJoin(argSlice)
		buffer.WriteString(fmt.Sprintf("F%sMethod::%s(args) => ", sName, mName))
		if len(method.Exceptions) == 0 {
			buffer.WriteString(fmt.Sprintf(`self
			.0
			.%s(&req.ctx,%s)
			.map(|res| F%sResponse::%s(F%s%sResult {`,
				methodName(method.Name), args, sName, mName, sName, mName))
			if method.ReturnType != nil {
				buffer.WriteString("success: Some(res),\n")
			}
			buffer.WriteString("})),\n")
		} else {
			buffer.WriteString(fmt.Sprintf(`match self.0.%s(&req.ctx,%s) {
				Ok(res) => Ok(F%sResponse::%s(F%s%sResult {`, methodName(method.Name), args, sName, mName, sName, mName))
			if method.ReturnType != nil {
				buffer.WriteString("success: Some(res),\n")
			}
			for _, exc := range method.Exceptions {
				buffer.WriteString(fmt.Sprintf("%s: None,\n", methodName(exc.Name)))
			}
			buffer.WriteString(fmt.Sprintf(`})),
				Err(err) => match err {
					thrift::Error::User(user_err) =>
						%s
						.map_err(|err| thrift::Error::User(err)),
				_ => Err(err)
			},
		},
		`, g.processorUserErrorHandling(sName, method)))
		}
	}
	buffer.WriteString(fmt.Sprintf(`};
			future::result(result)
		}
	}
	
	impl<S> FProcessor for F%sProcessor<S>
	where
		S: Service<Request = F%sRequest, Response = F%sResponse, Error = thrift::Error>
			+ Clone
			+ Send
			+ 'static,
	{
		fn process<R, W>(
			&mut self,
			iprot: &mut FInputProtocol<R>,
			oprot: &mut FOutputProtocol<W>,
		) -> thrift::Result<()>
		where
			R: thrift::transport::TReadTransport,
			W: thrift::transport::TWriteTransport,
		{
			let ctx = iprot.read_request_header()?;
			let name = {
				let mut iproxy = iprot.t_protocol_proxy();
				iproxy.read_message_begin().map(|tmid| tmid.name)?
			};

			match &*name {
		`, sName, sName, sName))
	for _, method := range s.Methods {
		buffer.WriteString(fmt.Sprintf("%q => self.%s(&ctx, iprot, oprot),\n", method.Name, methodName(method.Name)))
	}
	buffer.WriteString(fmt.Sprintf(`_ => {
					error!(
						"frugal: client invoked unknown function {} on request with correlation id {}",
						&name,
						ctx.correlation_id()
					);
					let mut iproxy = iprot.t_protocol_proxy();
					iproxy.skip(thrift::protocol::TType::Struct)?;
					iproxy.read_message_end()?;

					oprot.write_response_header(&ctx)?;
					let mut oproxy = oprot.t_protocol_proxy();
					oproxy.write_message_begin(&thrift::protocol::TMessageIdentifier::new(
						&name as &str,
						thrift::protocol::TMessageType::Exception,
						0,
					))?;
					let ex = thrift::ApplicationError::new(
						thrift::ApplicationErrorKind::UnknownMethod,
						format!("Unknown function {}", &name),
					);
					thrift::Error::write_application_error_to_out_protocol(&ex, &mut oproxy)?;
					oproxy.write_message_end()?;
					oproxy.flush()
				}
			}
		}
	}
	
	impl<S> F%sProcessor<S>
	where
		S: Service<Request = F%sRequest, Response = F%sResponse, Error = thrift::Error>,
	{
		`, sName, sName, sName))
	for _, method := range s.Methods {
		mName := typeName(method.Name)
		buffer.WriteString(fmt.Sprintf(`fn %s<R, W>(
			        &mut self,
			        ctx: &FContext,
			        iprot: &mut FInputProtocol<R>,
			        oprot: &mut FOutputProtocol<W>,
			    ) -> thrift::Result<()>
			    where
			        R: thrift::transport::TReadTransport,
			        W: thrift::transport::TWriteTransport,
			    {
			        let mut args = F%s%sArgs {};
			        let mut iproxy = iprot.t_protocol_proxy();
			        args.read(&mut iproxy)?;
			        iproxy.read_message_end()?;
			        let req = F%sRequest {
			            ctx: ctx.clone(),
			            method: F%sMethod::%s(args),
			        };
			        match self.service.call(req).wait() {
			            Err(thrift::Error::User(err)) => {
			                error!(
			                    "{} {}: {}",
			                    errors::USER_ERROR_DESCRIPTION,
			                    ctx.correlation_id(),
			                    err.description()
			                );
			                Ok(())
			            }
			            Err(err) => {
			                error!(
			                    "{} {}: {}",
			                    errors::USER_ERROR_DESCRIPTION,
			                    ctx.correlation_id(),
			                    err.description()
			                );
			                Ok(())
			            }
			            Ok(response) => {
			                let write_result = oprot
			                    .write_response_header(&ctx)
			                    .and_then(|()| {
			                        oprot.t_protocol_proxy().write_message_begin(
			                            &thrift::protocol::TMessageIdentifier::new(
			                                "basePing",
			                                thrift::protocol::TMessageType::Reply,
			                                0,
			                            ),
			                        )
			                    })
			                    .and_then(|()| {
			                        let result = F%s%sResult {};
			                        result.write(&mut oprot.t_protocol_proxy())
			                    })
			                    .and_then(|()| oprot.t_protocol_proxy().write_message_end())
			                    .and_then(|()| oprot.t_protocol_proxy().flush());
			
			                match write_result {
			                    Err(err) => {
			                        if errors::is_too_large_error(&err) {
			                            errors::write_application_error(
			                                "basePing",
			                                &ctx,
			                                &thrift::ApplicationError::new(
			                                    thrift::ApplicationErrorKind::Unknown,
			                                    errors::APPLICATION_EXCEPTION_RESPONSE_TOO_LARGE,
			                                ),
			                                oprot,
			                            )
			                        } else {
			                            Err(err)
			                        }
			                    }
			                    Ok(_) => Ok(()),
			                }
			            }
			        }
			    }
			`, methodName(method.Name), sName, mName, sName, sName, mName, sName, mName))
	}
	buffer.WriteString(`}`)

	_, err := file.Write(buffer.Bytes())
	return err
}

func methodToArgsStruct(serviceName string, method *parser.Method) *parser.Struct {
	return &parser.Struct{
		Comment:     []string{},
		Name:        fmt.Sprintf("F%s%sArgs", serviceName, typeName(method.Name)),
		Fields:      method.Arguments,
		Type:        parser.StructTypeStruct,
		Annotations: []*parser.Annotation{},
	}
}

func methodToResultStruct(serviceName string, method *parser.Method) *parser.Struct {
	fields := []*parser.Field{}
	if method.ReturnType != nil {
		fields = append(fields, &parser.Field{
			Comment:  []string{},
			ID:       0,
			Name:     "Success",
			Modifier: parser.Optional,
			Type:     method.ReturnType,
		})
	}
	fields = append(fields, method.Exceptions...)
	return &parser.Struct{
		Comment:     []string{},
		Name:        fmt.Sprintf("F%s%sResult", serviceName, typeName(method.Name)),
		Fields:      fields,
		Type:        parser.StructTypeStruct,
		Annotations: []*parser.Annotation{},
	}
}

func title(s string) string {
	if s == "" {
		return ""
	}

	if s == strings.ToUpper(s) {
		return s
	}

	return strings.Title(s)
}

func (g *Generator) canonicalizeTypeName(t *parser.Type) string {
	tName := t.Name
	if strings.Contains(tName, ".") {
		tName = strings.Split(tName, ".")[1]
		tName = typeName(tName)
		if t.IncludeName() != "" {
			includeName := t.IncludeName()
			if namespace := g.Frugal.NamespaceForInclude(t.IncludeName(), lang); namespace != nil {
				includeName = namespace.Value
			}
			tName = fmt.Sprintf("%s::%s", methodName(includeNameToReference(includeName)), tName)
		}
	} else {
		tName = typeName(tName)
	}
	return tName
}

func (g *Generator) toRustType(t *parser.Type, optional bool) string {
	if t == nil {
		return "()"
	}

	var name string
	switch t.Name {
	case "bool":
		name = "bool"
	case "byte":
		name = "u8"
	case "i8":
		name = "i8"
	case "i16":
		name = "i16"
	case "i32":
		name = "i32"
	case "i64":
		name = "i64"
	case "double":
		name = "OrderedFloat<f64>"
	case "string":
		name = "String"
	case "binary":
		name = "Vec<u8>"
	case "list":
		name = fmt.Sprintf("Vec<%s>", g.toRustType(t.ValueType, false))
	case "set":
		name = fmt.Sprintf("BTreeSet<%s>", g.toRustType(t.ValueType, false))
	case "map":
		name = fmt.Sprintf("BTreeMap<%s, %s>",
			g.toRustType(t.KeyType, false),
			g.toRustType(t.ValueType, false))
	}

	// otherwise we're a struct, enum, or union
	if name == "" {
		name = g.canonicalizeTypeName(t)
	}

	if optional {
		name = fmt.Sprintf("Option<%s>", name)
	}

	return name
}
