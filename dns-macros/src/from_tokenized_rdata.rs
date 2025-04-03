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
    let field_count = data.fields.len();
    let pattern_match = data.fields.iter().map(|f| &f.ident);
    let ignored_pattern_match = data.fields.iter().map(|_| quote! { _ });
    let field_name = data.fields.iter().map(|f| &f.ident);
    let field_type = data.fields.iter().map(|f| &f.ty);

    if data.fields.is_empty() {
        // Case 1: Struct has no fields.
        quote! {
            impl crate::serde::presentation::from_tokenized_rdata::FromTokenizedRData for #name {
                #[inline]
                fn from_tokenized_rdata(rdata: &Vec<&str>) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError> where Self: Sized {
                    match rdata.as_slice() {
                        &[] => Ok(Self {}),
                        &[..] => Err(crate::serde::presentation::errors::TokenizedRecordError::TooManyRDataTokensError(0, rdata.len())),
                    }
                }
            }
        }.into()
    } else {
        // Case 2: Struct has 1+ fields.
        quote! {
            impl crate::serde::presentation::from_tokenized_rdata::FromTokenizedRData for #name {
                #[inline]
                fn from_tokenized_rdata(rdata: &Vec<&str>) -> Result<Self, crate::serde::presentation::errors::TokenizedRecordError> where Self: Sized {
                    match rdata.as_slice() {
                        &[ #( #pattern_match),* ] => {
                            Ok(Self {
                                #( #field_name: <#field_type as crate::serde::presentation::from_presentation::FromPresentation>::from_token_format(&[#field_name])?.0 ),*
                            })
                        },
                        &[ #( #ignored_pattern_match, )* ..] => Err(crate::serde::presentation::errors::TokenizedRecordError::TooManyRDataTokensError{expected: #field_count, received: rdata.len()}),
                        &[..] => Err(crate::serde::presentation::errors::TokenizedRecordError::TooFewRDataTokensError{expected: #field_count, received: rdata.len()}),
                    }
                }
            }
        }.into()
    }
}
