use proc_macro2::Ident;
use quote::format_ident;
use std::collections::HashSet;
use syn::Attribute;

use crate::errors::Error;
use crate::utils::{
    get_simple_name_from_meta, process_bool_literal, process_enum_literal, process_string_literal,
};

pub const RELATIVE_IMPORTS_ATTRIBUTE: &str = "relative_imports";
pub const BUILD_ATTRIBUTE_PREFIX: &str = "build_";
pub const SKIP_IMPL_ATTRIBUTE: &str = "skip_impl";
pub const SKIP_FIELDS_ATTRIBUTE: &str = "skip_fields";
pub const SYNC_LEVEL_ATTRIBUTE: &str = "sync_level";
pub static SYNC_LEVEL_ATTRIBUTE_NAMES: &[&str] =
    &["document", "collection", "document_and_collection"];
pub const SYNC_COLLECTION_KEY_METHOD_ATTRIBUTE: &str = "sync_collection_key_method";
pub const COLLECTION_NAME_ATTRIBUTE: &str = "collection_name";
pub const COLLECTION_TYPE_ATTRIBUTE: &str = "collection_type";
pub const COLLECTION_KIND_ATTRIBUTE: &str = "collection_kind";

#[derive(Default)]
pub struct ModelOptions {
    pub relative_imports: bool,
    pub build_models: HashSet<String>,
    pub skip_impl: bool,
    pub skip_fields: bool,
    pub serialize_fields: bool,
    pub sync_level: SyncLevelType,
    pub sync_collection_key_method: Option<Ident>,
    pub collection_name: Option<Ident>,
    pub collection_type: Option<Ident>,
    pub collection_kind: Option<Ident>,
}

impl ModelOptions {
    // CONSTRUCTORS -----------------------------------------------------------

    pub fn from_attributes(attributes: &[Attribute]) -> Result<ModelOptions, syn::Error> {
        let mut result = ModelOptions::default();
        #[allow(clippy::never_loop)]
        // Read every attribute, i.e. #[...]
        for attribute in attributes {
            // Transform the attribute as meta, i.e. removing the brackets.
            let meta = &attribute.meta;

            // Get the name.
            let name = match get_simple_name_from_meta(meta) {
                Some(v) => v,
                None => return Err(Error::UnexpectedMacroOption.with_tokens(attribute)),
            };
            let name = name.as_str();

            match name {
                RELATIVE_IMPORTS_ATTRIBUTE => {
                    result.relative_imports = process_bool_literal(meta, name, Some(true))?;
                }
                SKIP_IMPL_ATTRIBUTE => {
                    result.skip_impl = process_bool_literal(meta, name, Some(true))?;
                }
                SKIP_FIELDS_ATTRIBUTE => {
                    result.skip_fields = process_bool_literal(meta, name, Some(true))?;
                }
                SYNC_LEVEL_ATTRIBUTE => {
                    static ENUM_LIST_VALUES: &[SyncLevelType] = &[
                        SyncLevelType::OnlyDocument,
                        SyncLevelType::OnlyCollection,
                        SyncLevelType::DocumentAndCollection,
                    ];

                    result.sync_level = process_enum_literal(
                        meta,
                        SYNC_LEVEL_ATTRIBUTE_NAMES,
                        ENUM_LIST_VALUES,
                        name,
                        Some(SyncLevelType::DocumentAndCollection),
                    )?;
                }
                SYNC_COLLECTION_KEY_METHOD_ATTRIBUTE => {
                    let value = process_string_literal(meta, name, None)?;
                    result.sync_collection_key_method = Some(format_ident!("{}", value));
                }
                COLLECTION_NAME_ATTRIBUTE => {
                    let value = process_string_literal(meta, name, None)?;
                    result.collection_name = Some(format_ident!("{}", value));
                }
                COLLECTION_TYPE_ATTRIBUTE => {
                    let value = process_string_literal(meta, name, None)?;
                    result.collection_type = Some(format_ident!("{}", value));
                }
                COLLECTION_KIND_ATTRIBUTE => {
                    let value = process_string_literal(meta, name, None)?;
                    result.collection_kind = Some(format_ident!("{}", value));
                }
                _ => {
                    if name.starts_with(BUILD_ATTRIBUTE_PREFIX) {
                        let final_name = name.trim_start_matches(BUILD_ATTRIBUTE_PREFIX);
                        result.build_models.insert(final_name.to_string());
                        continue;
                    }

                    return Err(Error::UnexpectedMacroOption.with_tokens(attribute));
                }
            }
        }

        Ok(result)
    }
}

// ----------------------------------------------------------------------------
// ----------------------------------------------------------------------------
// ----------------------------------------------------------------------------

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SyncLevelType {
    None,
    OnlyDocument,
    OnlyCollection,
    DocumentAndCollection,
}

impl SyncLevelType {
    // GETTERS ----------------------------------------------------------------

    pub fn is_document_active(&self) -> bool {
        *self == SyncLevelType::OnlyDocument || *self == SyncLevelType::DocumentAndCollection
    }

    pub fn is_collection_active(&self) -> bool {
        *self == SyncLevelType::OnlyCollection || *self == SyncLevelType::DocumentAndCollection
    }

    pub fn is_active(&self) -> bool {
        *self != SyncLevelType::None
    }
}

impl Default for SyncLevelType {
    fn default() -> Self {
        Self::None
    }
}
