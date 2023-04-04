use proc_macro2::TokenStream;
use quote::ToTokens;
use std::collections::HashMap;
use syn::Attribute;

use crate::utils::{get_simple_name_from_meta, process_bool_literal, process_only_attribute};

pub const ATTR_ATTRIBUTE_SUFFIX: &str = "_attr";
pub const SKIP_DEFAULT_ATTRIBUTE: &str = "skip_default";

#[derive(Default)]
pub struct StructAttributes {
    pub attributes: Vec<TokenStream>,
    pub attributes_by_model: HashMap<String, Vec<TokenStream>>,
    pub skip_default: bool,
}

impl StructAttributes {
    // CONSTRUCTORS -----------------------------------------------------------

    pub fn from_attributes(attributes: &[Attribute]) -> Result<StructAttributes, syn::Error> {
        let mut result = StructAttributes::default();

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
                SKIP_DEFAULT_ATTRIBUTE => {
                    result.skip_default = process_bool_literal(meta, name, Some(true))?;
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

                    result.attributes.push(attribute.to_token_stream());
                }
            }
        }

        Ok(result)
    }
}
