pub(crate) mod ast;
pub(crate) mod commit_folder;
mod constants;
mod decode_compiler;
mod encode_basic_repeated_type_field;
mod encode_basic_type_field;
mod encode_call;
mod encode_compiler;
mod encode_enum_field;
mod encode_map_field;
mod encode_message_expr;
mod ensure_import;
mod enum_compiler;
mod file_name_to_folder_name;
mod file_to_folder;
mod get_relative_import;
mod has_property;
mod is_reserved;
mod is_safe_id;
mod message_name_to_encode_type_name;
pub(crate) mod scope_to_folder;
mod render_file;
mod to_js_string;
mod ts_path;
mod types_compiler;
