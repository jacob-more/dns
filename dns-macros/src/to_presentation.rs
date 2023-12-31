use proc_macro;
use syn::{DeriveInput, Data, DataStruct};
use quote::quote;

pub fn impl_to_presentation_macro(ast: &DeriveInput) -> proc_macro::TokenStream {
    match &ast.data {
        Data::Struct(data) => impl_to_presentation_struct_macro(data, ast),
        Data::Enum(_) => panic!("Enum not implemented"),
        Data::Union(_) => panic!("Union not implemented"),
    }
}

fn impl_to_presentation_struct_macro(data: &DataStruct, ast: &DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;

    let mut to_token_calls = quote!{};
    let struct_declaration_builder = quote!{};
    for field in data.fields.iter() {
        let field_name = &field.ident;

        to_token_calls.extend(quote! {
            (self.#field_name as crate::serde::presentation::to_presentation::ToPresentation).to_presentation_format(out_buffer);
        });
    }

    let gen;
    if struct_declaration_builder.is_empty() {
        // Case 1: Struct has no fields.
        gen = quote! {
            impl crate::serde::presentation::to_presentation::ToPresentation for #name {
                #[inline]
                fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {}
            }
        };
    } else {
        // Case 2: Struct has 1+ fields.
        gen = quote! {
            impl crate::serde::presentation::to_presentation::ToPresentation for #name {
                #[inline]
                fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
                    #to_token_calls
                }
            }
        };
    }
    gen.into()
}
