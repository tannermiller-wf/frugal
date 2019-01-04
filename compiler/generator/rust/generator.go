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

	includedCrates := []string{}
	for _, include := range g.Frugal.Includes {
		namespace := g.Frugal.NamespaceForInclude(include.Name, lang)
		if namespace != nil {
			includedCrates = append(includedCrates, fmt.Sprintf("extern crate %s;", includeNameToReference(namespace.Value)))
		}
	}
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

func includeNameToReference(includeName string) string {
	split := strings.FieldsFunc(includeName, func(r rune) bool {
		return r == '.' || r == '/'
	})
	return split[len(split)-1]
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
	buffer.WriteString("#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]\n")
	buffer.WriteString(fmt.Sprintf("pub struct %s {\n", sName))
	constructorExpressions := make([]string, 0, len(s.Fields))
	for _, f := range s.Fields {
		g.writeDocComment(&buffer, f.Comment)
		g.writeAnnotations(&buffer, f.Annotations)
		t := g.toRustType(f.Type, f.Modifier != parser.Required)
		fieldName := methodName(f.Name)
		buffer.WriteString(fmt.Sprintf("pub %s: %s,\n", fieldName, t))

		// the following are needed in the impl block
		defaultValue := ""
		if f.Default != nil {
			defaultValue = g.generateRustLiteral(f.Type, f.Default, false)
			if f.Modifier == parser.Required {
				defaultValue = fmt.Sprintf("%s", defaultValue)
			} else {
				defaultValue = fmt.Sprintf("Some(%s)", defaultValue)
			}
			constructorExpressions = append(constructorExpressions, fmt.Sprintf("%s: %s,", fieldName, defaultValue))
		} else {
			if f.Modifier == parser.Required {
				constructorExpressions = append(constructorExpressions, fmt.Sprintf("%s: %s,", fieldName, g.zeroValue(f.Type)))
			} else {
				constructorExpressions = append(constructorExpressions, fmt.Sprintf("%s: None,", fieldName))
			}
		}
	}
	buffer.WriteString("}\n\n")

	// now the impl block
	buffer.WriteString(fmt.Sprintf("impl %s {\n", sName))

	// constructor method
	//buffer.WriteString(fmt.Sprintf("pub fn new() -> %s {\n", sName))
	//buffer.WriteString(fmt.Sprintf("%s {\n", sName))
	//buffer.WriteString(strings.Join(constructorExpressions, "\n"))
	//buffer.WriteString("}\n")
	//buffer.WriteString("}\n\n")

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
			`oprot.write_field_begin(&thrift::protocol::TFieldIdentifier {
				name: Some(%q.into()),
				field_type: thrift::protocol::TType::%s,
				id: Some(%d),
			})?;
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
			`oprot.write_list_begin(&thrift::protocol::TListIdentifier{
				element_type: thrift::protocol::TType::%s,
				size: %s.len() as i32,
			 })?;
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
			`oprot.write_set_begin(&thrift::protocol::TSetIdentifier{
				element_type: thrift::protocol::TType::%s,
				size: %s.len() as i32,
			 })?;
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
			`oprot.write_map_begin(&thrift::protocol::TMapIdentifier {
				key_type: Some(thrift::protocol::TType::%s),
				value_type: Some(thrift::protocol::TType::%s),
				size: %s.len() as i32,
			 })?;
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
		`pub fn read<T>(&mut self, iprot: &mut T) -> thrift::Result<()>
    	 where
    	     T: thrift::protocol::TInputProtocol,
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
			`fn read_field_%d<T>(&mut self, iprot: &mut T) -> thrift::Result<()>
    	 	 where
    	 	     T: thrift::protocol::TInputProtocol,
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
	// TODO: Handle other imports?
	_, err := file.WriteString(
		`#![allow(unused_variables)]

		 use std::collections::BTreeMap;
		 use std::error::Error;
		 
		 use futures::future::{self, FutureResult};
		 use futures::{Async, Future, Poll};
		 use thrift;
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
		 use frugal::service::example;
		 use frugal::service::Request;
		 use frugal::transport::FTransport;
		
		`,
	)
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
		args = append(args, fmt.Sprintf("%s: %s", f.Name, t))
	}
	return args
}

func (g *Generator) generateMethodSignature(method *parser.Method) string {
	return fmt.Sprintf("fn %s(&mut self, ctx: &FContext, %s) -> thrift::Result<%s>\n",
		methodName(method.Name), commaSpaceJoin(g.generateMethodArguments(method.Arguments)), g.toRustType(method.ReturnType, false))
}

func (g *Generator) GenerateService(file *os.File, s *parser.Service) error {
	var buffer bytes.Buffer

	// write the service trait
	g.writeDocComment(&buffer, s.Comment)
	g.writeAnnotations(&buffer, s.Annotations)
	sName := typeName(s.Name)
	extends := ""
	if s.Extends != "" {
		extends = fmt.Sprintf(": %s ", strings.Replace(s.Extends, ".", "::", -1))
	}
	buffer.WriteString(fmt.Sprintf("pub trait F%s%s {\n", sName, extends))
	for _, method := range s.Methods {
		g.writeDocComment(&buffer, method.Comment)
		g.writeAnnotations(&buffer, method.Annotations)
		buffer.WriteString(fmt.Sprintf("%s;\n", g.generateMethodSignature(method)))
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
		buffer.WriteString(fmt.Sprintf("%s(_) => %q,\n", typeName(method.Name), method.Name))
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

		impl Request for F%sRequest {
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

	// TODO: write the service client

	// TODO: write the service processor

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
			namespace := g.Frugal.NamespaceForInclude(t.IncludeName(), lang)
			if namespace != nil {
				tName = fmt.Sprintf("%s::%s", includeNameToReference(namespace.Value), tName)
			}
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
