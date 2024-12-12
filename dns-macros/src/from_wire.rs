use proc_macro;
use syn::{DeriveInput, Data, DataStruct};
use quote::quote;

pub fn impl_from_wire_macro(ast: &DeriveInput) -> proc_macro::TokenStream {
    match &ast.data {
        Data::Struct(data) => impl_from_wire_struct_macro(data, ast),
        Data::Enum(_) => panic!("Enum not implemented"),
        Data::Union(_) => panic!("Union not implemented"),
    }
}

fn impl_from_wire_struct_macro(data: &DataStruct, ast: &DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;

    let mut from_wire_calls = quote!{};
    let mut struct_declaration_builder = quote!{};
    for field in data.fields.iter() {
        let field_name = &field.ident;
        let field_type = &field.ty;

        from_wire_calls.extend(quote! {
            let #field_name = <#field_type>::from_wire_format(wire)?;
        });

        struct_declaration_builder.extend(quote!(
            #field_name: #field_name,
        ))
    }

    let gen;
    if struct_declaration_builder.is_empty() {
        // Case 1: Struct has no fields.
        gen = quote! {
            impl crate::serde::wire::from_wire::FromWire for #name {
                #[inline]
                fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
                    Ok(Self {})
                }
            }
        };
    } else {
        // Case 2: Struct has 1+ fields.
        gen = quote! {
            impl crate::serde::wire::from_wire::FromWire for #name {
                #[inline]
                fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
                    #from_wire_calls
                    Ok(Self { #struct_declaration_builder })
                }
            }
        };
    }
    gen.into()
}
