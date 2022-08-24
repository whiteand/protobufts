use std::rc::Rc;

use crate::proto::{
    compiler::ts::ast::{self, Type},
    error::ProtoError,
    package::{self, Declaration, FieldTypeReference, MessageDeclaration, MessageEntry},
    proto_scope::{root_scope::RootScope, ProtoScope},
};

use super::{
    ast::Folder,
    block_scope::BlockScope,
    constants::PROTOBUF_MODULE,
    defined_id::IdType,
    ensure_import::ensure_import,
    get_relative_import::get_relative_import,
    message_name_to_encode_type_name::message_name_to_encode_type_name,
    ts_path::{TsPath, TsPathComponent},
};

pub(super) fn insert_message_types(
    root: &RootScope,
    message_folder: &mut Folder,
    message_scope: &ProtoScope,
) -> Result<(), ProtoError> {
    let mut file = super::ast::File::new("types".into());

    insert_encoded_input_interface(&root, &mut file, &message_scope)?;
    insert_decode_result_interface(&root, &mut file, &message_scope)?;

    message_folder.entries.push(file.into());

    ///! TODO: Implement this
    Ok(())
}

fn insert_encoded_input_interface(
    root: &RootScope,
    types_file: &mut ast::File,
    message_scope: &ProtoScope,
) -> Result<(), ProtoError> {
    let message_name = message_scope.name();
    let mut interface = ast::InterfaceDeclaration::new_exported(
        message_name_to_encode_type_name(&message_name).into(),
    );
    let message_declaration = match message_scope {
        ProtoScope::Message(m) => m,
        _ => unreachable!(),
    };
    for entry in &message_declaration.entries {
        match entry {
            MessageEntry::Field(f) => {
                let property_type =
                    import_encoding_input_type(&root, &message_scope, types_file, &f.field_type)?
                        .or(&Type::Null);
                interface.members.push(
                    ast::PropertySignature::new_optional(f.json_name(), property_type).into(),
                );
            }
            MessageEntry::OneOf(_) => todo!(),
            // Field(f) => {
            // }
            // Declaration(_) => {}
            // OneOf(_) => todo!("Not implemented handling of OneOf Field"),
        }
    }

    types_file.ast.statements.push(interface.into());
    Ok(())
}

fn insert_decode_result_interface(
    root: &RootScope,
    types_file: &mut ast::File,
    message_scope: &ProtoScope,
) -> Result<(), ProtoError> {
    let mut interface = ast::InterfaceDeclaration::new_exported(message_scope.name().into());
    let message_declaration = match message_scope {
        ProtoScope::Message(m) => m,
        _ => unreachable!(),
    };
    for entry in &message_declaration.entries {
        use crate::proto::package::MessageEntry::*;
        match entry {
            Field(f) => {
                let property_type =
                    import_decode_result_type(&root, &message_scope, types_file, &f.field_type)?
                        .or(&Type::Null);
                interface
                    .members
                    .push(ast::PropertySignature::new_optional(f.json_name(), property_type).into())
            }
            OneOf(_) => todo!("Not implemented handling of OneOf Field"),
        }
    }

    types_file.ast.statements.push(interface.into());
    Ok(())
}

fn import_encoding_input_type(
    root: &RootScope,
    message_scope: &ProtoScope,
    types_file: &mut ast::File,
    field_type: &package::Type,
) -> Result<Type, ProtoError> {
    match field_type {
        package::Type::Enum(e_id) => import_enum_type(root, message_scope, types_file, *e_id),
        package::Type::Message(m_id) => {
            let imported_message_id = *m_id;
            let imported_name = Rc::from(message_name_to_encode_type_name(
                &root.get_declaration_name(imported_message_id).unwrap(),
            ));
            import_message_type(
                root,
                message_scope,
                types_file,
                field_type,
                imported_message_id,
                imported_name,
            )
        }
        package::Type::Repeated(field_type) => {
            let element_type =
                import_encoding_input_type(root, message_scope, types_file, field_type)?;
            return Ok(Type::array(element_type));
        }
        package::Type::Map(_, _) => todo!(),
        package::Type::Bool => Ok(Type::Boolean),
        package::Type::Bytes => Ok(Type::reference(ast::Identifier::new("Uint8Array").into())),
        package::Type::Double => Ok(Type::Number),
        package::Type::Fixed32 => Ok(Type::Number),
        package::Type::Fixed64 => Ok(Type::Number),
        package::Type::Float => Ok(Type::Number),
        package::Type::Int32 => Ok(Type::Number),
        package::Type::Int64
        | package::Type::Sfixed64
        | package::Type::Sint64
        | package::Type::Uint64 => {
            let util_id: Rc<ast::Identifier> = Rc::new("util".into());
            let util_import = ast::ImportDeclaration::import(
                vec![ast::ImportSpecifier::new(Rc::clone(&util_id))],
                PROTOBUF_MODULE.into(),
            );
            ensure_import(types_file, util_import);
            Ok(Type::TypeReference(vec![
                Rc::clone(&util_id),
                Rc::new(ast::Identifier::new("Long")),
            ])
            .or(&Type::Number))
        }
        package::Type::Sfixed32 => Ok(Type::Number),
        package::Type::Sint32 => Ok(Type::Number),
        package::Type::String => Ok(Type::String),
        package::Type::Uint32 => Ok(Type::Number),
        // FieldTypeReference::IdPath(ids) => {
        //     if ids.is_empty() {
        //         unreachable!();
        //     }
        //     let resolve_result = type_scope.resolve_path(ids)?;
        //     let requested_path = resolve_result.path();
        //     let mut requested_ts_path = TsPath::from(requested_path);

        //     let imported_type_name = match resolve_result.declaration {
        //         IdType::DataType(decl) => match decl {
        //             Declaration::Enum(e) => {
        //                 requested_ts_path.push(TsPathComponent::Enum(e.name.clone()));
        //                 Rc::clone(&e.name)
        //             }
        //             Declaration::Message(m) => {
        //                 requested_ts_path.push(TsPathComponent::File("types".into()));
        //                 let encode_type_name: Rc<str> =
        //                     Rc::from(message_name_to_encode_type_name(&m.name));
        //                 requested_ts_path
        //                     .push(TsPathComponent::Interface(Rc::clone(&encode_type_name)));
        //                 encode_type_name
        //             }
        //         },
        //         IdType::Package(_) => unreachable!(),
        //     };

        //     let mut current_file_path = TsPath::from(type_scope.path());
        //     current_file_path.push(TsPathComponent::File("types".into()));

        //     match get_relative_import(&current_file_path, &requested_ts_path) {
        //         Some(import_declaration) => {
        //             ensure_import(types_file, import_declaration);
        //         }
        //         _ => {}
        //     }

        //     return Ok(Type::reference(
        //         ast::Identifier {
        //             text: imported_type_name,
        //         }
        //         .into(),
        //     ));
        // }
        // FieldTypeReference::Repeated(field_type) => {
        //     let element_type = import_encoding_input_type(types_file, type_scope, field_type)?;
        //     return Ok(Type::array(element_type));
        // }
        // FieldTypeReference::Map(key, value) => {
        //     let key_type = import_encoding_input_type(types_file, type_scope, key)?;
        //     let value_type = import_encoding_input_type(types_file, type_scope, value)?;
        //     return Ok(Type::Record(Box::new(key_type), Box::new(value_type)));
        // }
    }
}

fn import_decode_result_type(
    root: &RootScope,
    message_scope: &ProtoScope,
    types_file: &mut ast::File,
    field_type: &package::Type,
) -> Result<Type, ProtoError> {
    match field_type {
        package::Type::Bool => Ok(Type::Boolean),
        package::Type::Bytes => Ok(Type::reference(ast::Identifier::new("Uint8Array").into())),
        package::Type::Double => Ok(Type::Number),
        package::Type::Fixed32 => Ok(Type::Number),
        package::Type::Fixed64 => Ok(Type::Number),
        package::Type::Float => Ok(Type::Number),
        package::Type::Int32 => Ok(Type::Number),
        package::Type::Int64
        | package::Type::Sfixed64
        | package::Type::Sint64
        | package::Type::Uint64 => {
            let util_id: Rc<ast::Identifier> = Rc::new("util".into());
            let util_import = ast::ImportDeclaration::import(
                vec![ast::ImportSpecifier::new(Rc::clone(&util_id))],
                PROTOBUF_MODULE.into(),
            );
            ensure_import(types_file, util_import);
            Ok(Type::TypeReference(vec![
                Rc::clone(&util_id),
                Rc::new(ast::Identifier::new("Long")),
            ]))
        }
        package::Type::Sfixed32 => Ok(Type::Number),
        package::Type::Sint32 => Ok(Type::Number),
        package::Type::String => Ok(Type::String),
        package::Type::Uint32 => Ok(Type::Number),
        package::Type::Enum(e_id) => import_enum_type(root, message_scope, types_file, *e_id),
        package::Type::Message(m_id) => {
            let message_id = *m_id;
            let imported_name = root.get_declaration_name(message_id).unwrap();
            import_message_type(
                root,
                message_scope,
                types_file,
                field_type,
                message_id,
                imported_name,
            )
        }
        package::Type::Repeated(field_type) => {
            let element_type =
            import_decode_result_type(root, message_scope, types_file, field_type)?;
            return Ok(Type::array(element_type));
        }
        package::Type::Map(_, _) => todo!(),
        // FieldTypeReference::IdPath(ids) => {
        //     if ids.is_empty() {
        //         unreachable!();
        //     }
        //     let resolve_result = scope.resolve_path(ids)?;
        //     let requested_path = resolve_result.path();
        //     let mut requested_ts_path = TsPath::from(requested_path);

        //     let mut imported_type_name = Rc::from(String::new());
        //     match resolve_result.declaration {
        //         IdType::DataType(decl) => match decl {
        //             Declaration::Enum(e) => {
        //                 requested_ts_path.push(TsPathComponent::Enum(Rc::clone(&e.name)));
        //                 imported_type_name = Rc::clone(&e.name);
        //             }
        //             Declaration::Message(m) => {
        //                 requested_ts_path.push(TsPathComponent::File("types".into()));
        //                 let encode_type_name = message_name_to_encode_type_name(&m.name);
        //                 imported_type_name = Rc::from(encode_type_name);
        //                 requested_ts_path
        //                     .push(TsPathComponent::Interface(Rc::clone(&imported_type_name)));
        //             }
        //         },
        //         IdType::Package(_) => unreachable!(),
        //     }

        //     let mut current_file_path = TsPath::from(scope.path());
        //     current_file_path.push(TsPathComponent::File("types".into()));

        //     match get_relative_import(&current_file_path, &requested_ts_path) {
        //         Some(import_declaration) => {
        //             ensure_import(types_file, import_declaration);
        //         }
        //         _ => {}
        //     }

        //     return Ok(Type::reference(
        //         ast::Identifier::new(&imported_type_name).into(),
        //     ));
        // }
        // FieldTypeReference::Repeated(field_type) => {
        //     let element_type = import_decode_result_type(types_file, scope, field_type)?;
        //     return Ok(Type::array(element_type));
        // }
        // FieldTypeReference::Map(key, value) => {
        //     let key_type = import_decode_result_type(types_file, scope, key)?;
        //     let value_type = import_decode_result_type(types_file, scope, value)?;
        //     return Ok(Type::Record(Box::new(key_type), Box::new(value_type)));
        // }
    }
}

fn import_enum_type(
    root: &RootScope,
    message_scope: &ProtoScope,
    types_file: &mut ast::File,
    enum_declaration_id: usize,
) -> Result<Type, ProtoError> {
    let enum_name = root.get_declaration_name(enum_declaration_id).unwrap();
    let enum_ts_path = {
        let enum_proto_path = root.get_declaration_path(enum_declaration_id).unwrap();
        let mut res = TsPath::from(enum_proto_path);
        res.push(TsPathComponent::Enum(Rc::clone(&enum_name)));
        res
    };
    let types_file_path = {
        let mut res = TsPath::from(
            root.get_declaration_path(message_scope.id().unwrap())
                .unwrap(),
        );
        res.push(TsPathComponent::File("types".into()));
        res
    };

    match get_relative_import(&types_file_path, &enum_ts_path) {
        Some(import_declaration) => {
            ensure_import(types_file, import_declaration);
        }
        _ => {}
    }

    return Ok(Type::reference(Rc::new(enum_name.into())));
}

fn import_message_type(
    root: &RootScope,
    message_scope: &ProtoScope,
    types_file: &mut ast::File,
    field_type: &package::Type,
    imported_message_id: usize,
    imported_name: Rc<str>,
) -> Result<Type, ProtoError> {
    let requested_ts_path = {
        let mut res = TsPath::from(root.get_declaration_path(imported_message_id).unwrap());
        res.push(TsPathComponent::File("types".into()));
        res.push(TsPathComponent::Interface(Rc::clone(&imported_name)));
        res
    };
    let current_file_path = {
        let current_message_path = root
            .get_declaration_path(message_scope.id().unwrap())
            .unwrap();
        let mut res = TsPath::from(current_message_path);
        res.push(TsPathComponent::File("types".into()));
        res
    };

    match get_relative_import(&current_file_path, &requested_ts_path) {
        Some(import_declaration) => {
            ensure_import(types_file, import_declaration);
        }
        _ => {}
    }

    return Ok(Type::reference(
        ast::Identifier {
            text: imported_name,
        }
        .into(),
    ));
}
