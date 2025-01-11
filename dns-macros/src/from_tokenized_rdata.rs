use proc_macro;
use syn::{DeriveInput, Data, DataStruct};
use quote::quote;

pub fn impl_from_tokenized_rdata_macro(ast: &DeriveInput) -> proc_macro::TokenStream {
    match &ast.data {
        Data::Struct(data) => impl_from_tokenized_rdata_struct_macro(data, ast),
        Data::Enum(_) => panic!("Enum not implemented"),
        Data::Union(_) => panic!("Union not implemented"),
    }
}

fn impl_from_tokenized_rdata_struct_macro(data: &DataStruct, ast: &DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;

    let mut from_token_calls = quote!{};
    let mut pattern_match = quote!{};
    let mut ignored_pattern_match = quote!{};
    let mut struct_declaration_builder = quote!{};
    let field_count = data.fields.len();
    for field in data.fields.iter() {
        let field_name = &field.ident;
        let field_type = &field.ty;

        pattern_match.extend(quote! {
            #field_name,
        });

        ignored_pattern_match.extend(quote! {
            _,
        });

        from_token_calls.extend(quote! {
            let (#field_name, _) = <#field_type as crate::serde::presentation::from_presentation::FromPresentation>::from_token_format(&[#field_name])?;
        });

        struct_declaration_builder.extend(quote!(
            #field_name: #field_name,
        ))
    }

    let gen;
    if struct_declaration_builder.is_empty() {
        // Case 1: Struct has no fields.
        gen = quote! {
            impl crate::serde::presentation::from_tokenized_rdata::FromTokenizedRData for #name {
                #[inline]
                fn from_tokenized_rdata(rdata: &Vec<&str>) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError> where Self: Sized {
                    match rdata.as_slice() {
                        &[] => Ok(Self {}),
                        &[..] => Err(crate::serde::presentation::errors::TokenizedRecordError::TooManyRDataTokensError(0, rdata.len())),
                    }
                }
            }
        };
    } else {
        // Case 2: Struct has 1+ fields.
        gen = quote! {
            impl crate::serde::presentation::from_tokenized_rdata::FromTokenizedRData for #name {
                #[inline]
                fn from_tokenized_rdata(rdata: &Vec<&str>) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError> where Self: Sized {
                    match rdata.as_slice() {
                        &[#pattern_match] => {
                            #from_token_calls
                            Ok(Self {
                                #struct_declaration_builder
                            })
                        },
                        &[#ignored_pattern_match ..] => Err(crate::serde::presentation::errors::TokenizedRecordError::TooManyRDataTokensError{expected: #field_count, received: rdata.len()}),
                        &[..] => Err(crate::serde::presentation::errors::TokenizedRecordError::TooFewRDataTokensError{expected: #field_count, received: rdata.len()}),
                    }
                }
            }
        };
    }
    gen.into()
}
