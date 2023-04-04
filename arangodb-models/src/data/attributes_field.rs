use proc_macro2::TokenStream;
use quote::ToTokens;
use std::collections::{HashMap, HashSet};
use syn::{Attribute, Type};

use crate::utils::{
    get_simple_name_from_meta, process_enum_literal, process_only_attribute, process_string_literal,
};

pub const ATTR_ATTRIBUTE_SUFFIX: &str = "_attr";
pub const SKIP_IN_ATTRIBUTE_PREFIX: &str = "skip_in_";
pub const DB_NAME_ATTRIBUTE: &str = "db_name";
pub const INNER_MODEL_ATTRIBUTE: &str = "inner_model";
pub static INNER_MODEL_ATTRIBUTE_NAMES: &[&str] = &["data", "struct", "enum"];
pub static INNER_MODEL_ATTRIBUTE_VALUES: &[InnerModelKind] = &[
    InnerModelKind::Data,
    InnerModelKind::Struct,
    InnerModelKind::Enum,
];
pub const INNER_TYPE_ATTRIBUTE_PREFIX: &str = "inner_type_";

#[derive(Default)]
pub struct FieldAttributes {
    pub attributes: Vec<TokenStream>,
    pub attributes_by_model: HashMap<String, Vec<TokenStream>>,
    pub skip_in_model: HashSet<String>,
    pub db_name: Option<String>,
    pub inner_model: InnerModelKind,
    pub inner_type_by_model: HashMap<String, Type>,
}

impl FieldAttributes {
    // CONSTRUCTORS -----------------------------------------------------------

    pub fn from_attributes(
        attributes: &[Attribute],
        in_enum: bool,
    ) -> Result<FieldAttributes, syn::Error> {
        let mut result = FieldAttributes::default();

        if in_enum {
            result.inner_model = InnerModelKind::Struct;
        }

        // Read every attribute, i.e. #[...]
        for attribute in attributes {
            // Transform the attribute as meta, i.e. removing the brackets.
            let meta = &attribute.meta;

            // Get the name.
            let name = match get_simple_name_from_meta(meta) {
                Some(v) => v,
                None => {
                    result.attributes.push(attribute.to_token_stream());
                    continue;
                }
            };
            let name = name.as_str();

            match name {
                DB_NAME_ATTRIBUTE => {
                    result.db_name = Some(process_string_literal(meta, name, None)?);
                }
                INNER_MODEL_ATTRIBUTE => {
                    result.inner_model = process_enum_literal(
                        meta,
                        INNER_MODEL_ATTRIBUTE_NAMES,
                        INNER_MODEL_ATTRIBUTE_VALUES,
                        name,
                        None,
                    )?;
                }
                _ => {
                    if name.ends_with(ATTR_ATTRIBUTE_SUFFIX) {
                        let final_name = name.trim_end_matches(ATTR_ATTRIBUTE_SUFFIX);
                        let value = process_only_attribute(meta, name)?;

                        match result.attributes_by_model.get_mut(final_name) {
                            Some(v) => {
                                v.push(value);
                            }
                            None => {
                                result
                                    .attributes_by_model
                                    .insert(final_name.to_string(), vec![value]);
                            }
                        }
                        continue;
                    }

                    if name.starts_with(SKIP_IN_ATTRIBUTE_PREFIX) {
                        let final_name = name.trim_start_matches(SKIP_IN_ATTRIBUTE_PREFIX);
                        result.skip_in_model.insert(final_name.to_string());
                        continue;
                    }

                    if name.starts_with(INNER_TYPE_ATTRIBUTE_PREFIX) {
                        let final_name = name.trim_start_matches(INNER_TYPE_ATTRIBUTE_PREFIX);
                        let value = process_string_literal(meta, name, None)?;
                        let value = syn::parse_str(&value)?;

                        result
                            .inner_type_by_model
                            .insert(final_name.to_string(), value);
                        continue;
                    }

                    result.attributes.push(attribute.to_token_stream());
                    continue;
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
pub enum InnerModelKind {
    Data,
    Struct,
    Enum,
}

impl Default for InnerModelKind {
    fn default() -> Self {
        Self::Data
    }
}
