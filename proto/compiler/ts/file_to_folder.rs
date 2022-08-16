use std::ops::Deref;

use super::ast::Folder;
use super::file_name_to_folder_name::file_name_to_folder_name;
use crate::proto::{
    compiler::ts::ast::*,
    error::ProtoError,
    package::{
        Declaration, EnumDeclaration, FieldType, ImportPath, MessageDeclaration, MessageEntry,
        ProtoFile,
    },
    package_tree::PackageTree,
    scope::Scope,
};

#[derive(Debug)]
struct BlockScope<'a> {
    root: &'a PackageTree,
    proto_file: &'a ProtoFile,
    parent_messages: Vec<&'a MessageDeclaration>,
}

impl<'scope> BlockScope<'scope> {
    pub fn push(&self, message: &'scope MessageDeclaration) -> BlockScope<'scope> {
        let mut parent_messages = vec![message];
        for p in self.parent_messages.iter() {
            parent_messages.push(p);
        }
        BlockScope {
            root: self.root,
            proto_file: self.proto_file,
            parent_messages,
        }
    }

    pub fn new<'x>(root: &'x PackageTree, proto_file: &'x ProtoFile) -> BlockScope<'x> {
        BlockScope {
            root,
            proto_file,
            parent_messages: Vec::new(),
        }
    }
    fn path(&self) -> ProtoPath {
        let mut res = ProtoPath::new();

        for package in self.proto_file.path.iter() {
            res.push(PathComponent::Package(package.clone()));
        }
        res.push(PathComponent::File(self.proto_file.name.clone()));
        for m in self.parent_messages.iter().rev() {
            res.push(PathComponent::Message(m.name.clone()));
        }

        res
    }
}

#[derive(Debug)]
enum IdType<'scope> {
    DataType(&'scope Declaration),
    Package(&'scope ProtoFile),
}

#[derive(Debug)]
struct DefinedId<'a> {
    scope: BlockScope<'a>,
    declaration: IdType<'a>,
}

impl<'scope> DefinedId<'scope> {
    fn resolve(&self, name: &str) -> Result<DefinedId<'scope>, ProtoError> {
        match self.declaration {
            IdType::DataType(decl) => match decl {
                Declaration::Enum(e) => {
                    return Err(self
                        .scope
                        .error(format!("Cannot resolve {}\n  in {}", name, e.name).as_str()))
                }
                Declaration::Message(m) => match m.resolve(name) {
                    Some(declaration) => Ok(DefinedId {
                        declaration: IdType::DataType(declaration),
                        scope: self.scope.push(m),
                    }),
                    None => Err(self
                        .scope
                        .error(format!("Cannot resolve {}\n  in {}", name, m.name).as_str())),
                },
            },
            IdType::Package(p) => {
                let package_block_scope = BlockScope {
                    root: self.scope.root,
                    parent_messages: Vec::new(),
                    proto_file: p,
                };
                return package_block_scope.resolve(name);
            }
        }
    }

    fn path(&self) -> ProtoPath {
        use PathComponent::*;
        let mut res = self.scope.path();
        match self.declaration {
            IdType::DataType(decl) => match decl {
                Declaration::Enum(e) => {
                    res.push(Enum(e.name.clone()));
                }
                Declaration::Message(m) => {
                    res.push(Message(m.name.clone()));
                }
            },
            IdType::Package(p) => {
                res.push(PathComponent::Package(p.name.clone()));
            }
        }
        res
    }
}

#[derive(Debug, Clone)]
enum PathComponent {
    Package(String),
    File(String),
    Message(String),
    Enum(String),
}

impl From<&PathComponent> for String {
    fn from(p: &PathComponent) -> String {
        match p {
            PathComponent::Package(s) => s.clone(),
            PathComponent::File(s) => s.clone(),
            PathComponent::Message(s) => s.clone(),
            PathComponent::Enum(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct ProtoPath {
    path: Vec<PathComponent>,
}

impl ProtoPath {
    fn new() -> Self {
        ProtoPath { path: Vec::new() }
    }
    fn push(&mut self, item: PathComponent) {
        self.path.push(item);
    }
    fn is_empty(&self) -> bool {
        self.path.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TsPathComponent {
    Folder(String),
    File(String),
    Enum(String),
    Interface(String),
    Function(String),
}
impl From<&TsPathComponent> for String {
    fn from(p: &TsPathComponent) -> String {
        match p {
            TsPathComponent::Folder(s) => s.clone(),
            TsPathComponent::File(s) => s.clone(),
            TsPathComponent::Enum(s) => s.clone(),
            TsPathComponent::Interface(s) => s.clone(),
            TsPathComponent::Function(s) => s.clone(),
        }
    }
}

impl TsPathComponent {
    fn is_declaration(&self) -> bool {
        match self {
            TsPathComponent::Folder(_) => false,
            TsPathComponent::File(_) => false,
            TsPathComponent::Enum(_) => true,
            TsPathComponent::Interface(_) => true,
            TsPathComponent::Function(_) => true,
        }
    }
    fn is_file(&self) -> bool {
        return matches!(self, TsPathComponent::File(_));
    }
    fn is_folder(&self) -> bool {
        return matches!(self, TsPathComponent::Folder(_));
    }
}

#[derive(Debug)]
struct TsPath {
    path: Vec<TsPathComponent>,
}

impl Deref for TsPath {
    type Target = Vec<TsPathComponent>;
    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl TsPath {
    fn file_path(&self) -> Self {
        let mut res = Self::default();
        for x in self.path.iter() {
            if x.is_declaration() {
                break;
            }
            res.push(x.clone());
        }
        res
    }
    fn push(&mut self, item: TsPathComponent) {
        self.path.push(item);
    }
}

impl Default for TsPath {
    fn default() -> Self {
        TsPath { path: Vec::new() }
    }
}

impl std::fmt::Display for TsPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.path.is_empty() {
            return Ok(());
        }
        for (prev, cur) in self.path.iter().zip(self.path[1..].iter()) {
            match (prev, cur) {
                (TsPathComponent::Folder(prev), _) => write!(f, "{}/", prev)?,
                (TsPathComponent::File(prev), _) => write!(f, "{}::", prev)?,
                (_, _) => unreachable!(),
            }
        }
        let str: String = self.path.last().unwrap().into();
        f.write_str(&str);
        Ok(())
    }
}

fn proto_path_to_ts_path(proto_path: ProtoPath) -> TsPath {
    let mut res = TsPath::default();
    if proto_path.is_empty() {
        return res;
    }
    let ProtoPath { path } = proto_path;
    for p in path.iter() {
        match p {
            PathComponent::Package(s) => {
                res.path.push(TsPathComponent::Folder(s.clone()));
            }
            PathComponent::File(s) => {
                res.path
                    .push(TsPathComponent::Folder(file_name_to_folder_name(s)));
            }
            PathComponent::Message(s) => {
                res.path.push(TsPathComponent::Folder(s.clone()));
            }
            PathComponent::Enum(s) => {
                res.path.push(TsPathComponent::File(s.clone()));
            }
        }
    }
    res
}

impl std::fmt::Display for ProtoPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            return Ok(());
        }
        for (prev, cur) in self.path.iter().zip(self.path[1..].iter()) {
            match (prev, cur) {
                (PathComponent::Package(prev), _) => write!(f, "{}/", prev)?,
                (PathComponent::File(prev), _) => write!(f, "{}::", prev)?,
                (PathComponent::Enum(_), _) => unreachable!(),
                (PathComponent::Message(prev), _) => write!(f, "{}.", prev)?,
            }
        }
        let str: String = self.path.last().unwrap().into();
        Ok(())
    }
}

impl std::fmt::Display for IdType<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdType::DataType(d) => write!(f, "{}", d),
            IdType::Package(proto_file) => write!(f, "{}", proto_file),
        }
    }
}

impl<'context> BlockScope<'context> {
    fn stack_trace(&self) -> Vec<String> {
        let mut res: Vec<String> = Vec::new();
        for &parent in self.parent_messages.iter() {
            res.push(parent.name.clone());
        }
        res.push(self.proto_file.full_path());
        res
    }
    fn print_stack_trace(&self) {
        for location in self.stack_trace() {
            println!(" in {}", location);
        }
    }
    fn resolve(&self, name: &str) -> Result<DefinedId<'context>, ProtoError> {
        for parent_index in 0..self.parent_messages.len() {
            let parent = self.parent_messages[parent_index];

            if let Some(declaration) = parent.resolve(name) {
                let parent_messages = self.parent_messages[parent_index..].to_vec();
                return Ok(DefinedId {
                    scope: BlockScope {
                        root: self.root,
                        proto_file: self.proto_file,
                        parent_messages,
                    },
                    declaration: IdType::DataType(declaration),
                });
            }
        }
        if let Some(declaration) = self.proto_file.resolve(name) {
            return Ok(DefinedId {
                scope: BlockScope {
                    root: self.root,
                    proto_file: self.proto_file,
                    parent_messages: Vec::new(),
                },
                declaration: IdType::DataType(declaration),
            });
        }

        'nextImport: for imprt in &self.proto_file.imports {
            let ImportPath {
                packages,
                file_name,
            } = imprt;

            if imprt.packages.last().unwrap().ne(name) {
                continue 'nextImport;
            }

            let mut root_path = self.proto_file.path.clone();

            loop {
                for package in packages {
                    root_path.push(package.clone());
                }
                match self.root.resolve_subtree(&root_path) {
                    Some(subtree) => {
                        match subtree.files.iter().find(|f| f.name == *file_name) {
                            Some(file) => {
                                return Ok(DefinedId {
                                    scope: BlockScope {
                                        root: self.root,
                                        proto_file: self.proto_file,
                                        parent_messages: Vec::new(),
                                    },
                                    declaration: IdType::Package(file),
                                });
                            }
                            None => {
                                continue 'nextImport;
                            }
                        };
                    }
                    None => {
                        for _ in packages {
                            root_path.pop();
                        }
                        if root_path.is_empty() {
                            continue 'nextImport;
                        }
                        root_path.pop();
                    }
                }
            }
        }

        return Err(self.error(format!("Could not resolve name {}", name).as_str()));
    }

    fn resolve_path(&self, path: &Vec<String>) -> Result<DefinedId, ProtoError> {
        let mut resolution = self.resolve(&path[0])?;
        for name in &path[1..] {
            resolution = resolution.resolve(name)?;
        }
        Ok(resolution)
    }

    fn error(&self, message: &str) -> ProtoError {
        let mut error_message = String::new();
        error_message.push_str(message);
        for location in self.stack_trace() {
            error_message.push_str(format!("\n  in {}", location).as_str());
        }
        return ProtoError::new(error_message);
    }
}

pub(super) fn file_to_folder(
    root: &PackageTree,
    package_tree: &PackageTree,
    file: &ProtoFile,
) -> Result<Folder, ProtoError> {
    let folder_name = file_name_to_folder_name(&file.name);
    let mut res = Folder::new(folder_name);
    for declaration in &file.declarations {
        match declaration {
            Declaration::Enum(enum_declaration) => {
                insert_enum_declaration(&mut res, enum_declaration);
            }
            Declaration::Message(message_declaration) => {
                let file_scope = BlockScope::new(root, file);
                insert_message_declaration(&mut res, file_scope, message_declaration)?;
            }
        }
    }
    Ok(res)
}

fn insert_message_declaration(
    message_parent_folder: &mut Folder,
    scope: BlockScope,
    message_declaration: &MessageDeclaration,
) -> Result<(), ProtoError> {
    let mut message_folder = Folder::new(message_declaration.name.clone());
    insert_message_types(&mut message_folder, &scope, message_declaration)?;
    insert_encode(&mut message_folder, &scope, message_declaration)?;
    insert_decode(&mut message_folder, &scope, message_declaration)?;
    insert_children(&mut message_folder, &scope, message_declaration)?;
    message_parent_folder.entries.push(message_folder.into());

    Ok(())
}

fn insert_message_types(
    message_folder: &mut Folder,
    scope: &BlockScope,
    message_declaration: &MessageDeclaration,
) -> Result<(), ProtoError> {
    let mut file = super::ast::File::new("types".into());

    insert_encoded_input_interface(&mut file, scope, message_declaration)?;
    insert_decode_result_interface(&mut file, scope, message_declaration)?;

    message_folder.entries.push(file.into());

    ///! TODO: Implement this
    Ok(())
}

fn message_name_to_encode_type_name(message_name: &str) -> String {
    format!("{}EncodeInput", message_name)
}

fn insert_encoded_input_interface(
    types_file: &mut super::ast::File,
    scope: &BlockScope,
    message_declaration: &MessageDeclaration,
) -> Result<(), ProtoError> {
    let mut interface = InterfaceDeclaration::new_exported(message_name_to_encode_type_name(
        &message_declaration.name,
    ));
    for entry in &message_declaration.entries {
        use crate::proto::package::MessageEntry::*;
        match entry {
            Field(f) => {
                let type_scope = scope.push(message_declaration);
                let t: Type = import_encoding_input_type(types_file, &type_scope, &f.field_type)?;
                interface
                    .members
                    .push(PropertySignature::new_optional(f.json_name(), t).into());
            }
            Declaration(_) => {}
            OneOf(_) => todo!("Not implemented handling of OneOf Field"),
        }
    }

    types_file.ast.statements.push(interface.into());
    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum TypeUsage {
    EncodingFieldType,
    DecodingFieldType,
}

fn try_get_predefined_type(s: &str) -> Option<Type> {
    match s {
        "bool" => Some(Type::Boolean),
        "string" => Some(Type::String),
        "int32" => Some(Type::Number),
        "uint32" => Some(Type::Number),
        "float" => Some(Type::Number),
        "bytes" => Some(Type::TypeReference("Uint8Array".into())),
        _ => None,
    }
}

fn import_encoding_input_type(
    types_file: &mut File,
    scope: &BlockScope,
    field_type: &FieldType,
) -> Result<Type, ProtoError> {
    match field_type {
        FieldType::IdPath(ids) => {
            if ids.is_empty() {
                unreachable!();
            }
            if ids.len() == 1 {
                match try_get_predefined_type(&ids[0]) {
                    Some(t) => return Ok(t),
                    None => {}
                }
            }
            let resolve_result = scope.resolve_path(ids)?;
            let requested_path = resolve_result.path();
            let mut requested_ts_path = proto_path_to_ts_path(requested_path);

            match resolve_result.declaration {
                IdType::DataType(decl) => match decl {
                    Declaration::Enum(e) => {
                        requested_ts_path.push(TsPathComponent::Enum(e.name.clone()));
                    }
                    Declaration::Message(m) => {
                        requested_ts_path.push(TsPathComponent::File("types".into()));
                        requested_ts_path.push(TsPathComponent::Interface(
                            message_name_to_encode_type_name(&m.name),
                        ));
                    }
                },
                IdType::Package(_) => unreachable!(),
            }

            let mut current_file_path = proto_path_to_ts_path(scope.path());
            current_file_path.push(TsPathComponent::File("types".into()));

            let import_declaration = get_relative_import(&current_file_path, &requested_ts_path);

            let str: String = (&import_declaration).into();
            println!("{}", str);
            dbg!(import_declaration);

            return Ok(Type::Null);
        }
        FieldType::Repeated(field_type) => {
            let element_type = import_encoding_input_type(types_file, scope, field_type)?;
            return Ok(Type::array(element_type));
        }
        FieldType::Map(key, value) => {
            let key_type = import_encoding_input_type(types_file, scope, key)?;
            let value_type = import_encoding_input_type(types_file, scope, value)?;
            return Ok(Type::Record(Box::new(key_type), Box::new(value_type)));
        }
    }
}

fn get_relative_import(
    mut from: &[TsPathComponent],
    mut to: &[TsPathComponent],
) -> ImportDeclaration {
    assert!(to.last().unwrap().is_declaration());
    while from.len() > 0 && to.len() > 0 && from[0] == to[0] {
        from = &from[1..];
        to = &to[1..];
    }
    assert!(from.len() > 0);
    assert!(to.len() > 0);
    let imported_component = to.last().unwrap();
    assert!(imported_component.is_declaration());
    let imported_name: String = imported_component.into();
    if from.first().unwrap().is_file() {
        let mut file_string = format!(".");
        for component in to.iter() {
            if component.is_declaration() {
                break;
            }
            file_string.push('/');
            let component_name: String = component.into();
            file_string.push_str(&component_name);
        }

        return ImportDeclaration {
            import_clause: ImportClause {
                name: None,
                named_bindings: Some(NamedImports {
                    elements: vec![ImportSpecifier::new(imported_name.into())],
                }),
            }
            .into(),
            string_literal: file_string.into(),
        };
    }

    let mut import_string = String::new();

    while from.len() > 0 && from[0].is_folder() {
        import_string.push_str("../");
        from = &from[1..];
    }

    while to.len() > 0 && to[0].is_folder() {
        let ref folder = to[0];
        let folder_name: String = folder.into();
        import_string.push_str(&folder_name);
        import_string.push('/');
        to = &to[1..];
    }
    let ref file_component = to[0];
    assert!(file_component.is_file());
    let file_name: String = file_component.into();
    import_string.push_str(&file_name);
    ImportDeclaration {
        import_clause: ImportClause {
            name: None,
            named_bindings: Some(NamedImports {
                elements: vec![ImportSpecifier::new(imported_name.into())],
            }),
        }
        .into(),
        string_literal: import_string.into(),
    }
}

#[cfg(test)]
mod test_get_relative_import {
    use super::get_relative_import;
    use super::TsPathComponent::*;
    #[test]
    #[should_panic]
    fn same_file_import_panics() {
        let from = &[File("types".into())];
        let to = &[File("types".into()), Enum("Test".into())];
        let _ = get_relative_import(from, to);
    }
    #[test]
    #[should_panic]
    fn same_file_import_panics_2() {
        let from = &[Folder("Hello".into()), File("types".into())];
        let to = &[
            Folder("Hello".into()),
            File("types".into()),
            Enum("Test".into()),
        ];
        let _ = get_relative_import(from, to);
    }
    #[test]
    fn same_folder_file() {
        let from = &[Folder("Hello".into()), File("types".into())];
        let to = &[
            Folder("Hello".into()),
            File("defs".into()),
            Enum("Test".into()),
        ];
        let decl = get_relative_import(from, to);
        let decl_str: String = (&decl).into();
        assert_eq!(decl_str, "import { Test } from \"./defs\"")
    }
    #[test]
    fn parent_folder_path() {
        let from = &[Folder("Goodbye".into()), File("types".into())];
        let to = &[
            Folder("Hello".into()),
            File("defs".into()),
            Enum("Test".into()),
        ];
        let decl = get_relative_import(from, to);
        let decl_str: String = (&decl).into();
        assert_eq!(decl_str, "import { Test } from \"../Hello/defs\"")
    }
}

fn insert_decode_result_interface(
    types_file: &mut super::ast::File,
    scope: &BlockScope,
    message_declaration: &MessageDeclaration,
) -> Result<(), ProtoError> {
    let mut interface = InterfaceDeclaration::new_exported(message_declaration.name.clone().into());
    for entry in &message_declaration.entries {
        use crate::proto::package::MessageEntry::*;
        match entry {
            Field(f) => interface
                .members
                .push(PropertySignature::new(f.json_name(), Type::Null).into()),
            Declaration(_) => {}
            OneOf(_) => todo!("Not implemented handling of OneOf Field"),
        }
    }

    types_file.ast.statements.push(interface.into());
    Ok(())
}

fn insert_encode(
    message_folder: &mut Folder,
    scope: &BlockScope,
    message_declaration: &MessageDeclaration,
) -> Result<(), ProtoError> {
    let file = super::ast::File::new("encode".into());

    message_folder.entries.push(file.into());

    ///! TODO: Implement this
    Ok(())
}
fn insert_decode(
    message_folder: &mut Folder,
    scope: &BlockScope,
    message_declaration: &MessageDeclaration,
) -> Result<(), ProtoError> {
    let file = super::ast::File::new("decode".into());

    message_folder.entries.push(file.into());
    Ok(())
}
fn insert_children(
    message_folder: &mut Folder,
    scope: &BlockScope,
    message_declaration: &MessageDeclaration,
) -> Result<(), ProtoError> {
    for entry in message_declaration.entries.iter() {
        match entry {
            MessageEntry::Field(_) => {}
            MessageEntry::OneOf(_) => {}
            MessageEntry::Declaration(decl) => match decl {
                Declaration::Enum(e) => {
                    insert_enum_declaration(message_folder, e);
                }
                Declaration::Message(m) => {
                    let mut child_context = BlockScope {
                        parent_messages: vec![message_declaration],
                        root: scope.root,
                        proto_file: scope.proto_file,
                    };
                    for p in scope.parent_messages.iter() {
                        child_context.parent_messages.push(p);
                    }

                    insert_message_declaration(message_folder, child_context, m)?;
                }
            },
        }
    }
    Ok(())
}

fn insert_enum_declaration(res: &mut Folder, enum_declaration: &EnumDeclaration) {
    let mut file = File::new(enum_declaration.name.clone());
    let enum_declaration = super::ast::EnumDeclaration {
        modifiers: vec![Modifier::Export],
        name: enum_declaration.name.clone().into(),
        members: enum_declaration
            .entries
            .iter()
            .map(|entry| super::ast::EnumMember {
                name: entry.name.clone().into(),
                value: Some(entry.value.into()),
            })
            .collect(),
    };
    file.ast.statements.push(enum_declaration.into());
    res.entries.push(file.into());
}
