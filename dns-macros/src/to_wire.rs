use proc_macro;
use syn::{DeriveInput, Data, DataStruct};
use quote::quote;

pub fn impl_to_wire_macro(ast: &DeriveInput) -> proc_macro::TokenStream {
    match &ast.data {
        Data::Struct(data) => impl_to_wire_struct_macro(data, ast),
        Data::Enum(_) => panic!("Enum not implemented"),
        Data::Union(_) => panic!("Union not implemented"),
    }
}

fn impl_to_wire_struct_macro(data: &DataStruct, ast: &DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;

    let mut to_wire_calls = quote!{};
    let mut serial_len_calls = quote!{};
    for (index, field) in data.fields.iter().enumerate() {
        let field_name = &field.ident;

        to_wire_calls.extend(quote! {
            self.#field_name.to_wire_format(wire, compression)?;
        });

        // The first line does not start with a "+" sign since it is the one that others are being
        // added on to it.
        if index != 0 {
            serial_len_calls.extend(quote! { + });
        }
        serial_len_calls.extend(quote! {
            self.#field_name.serial_length()
        });
    }

    let gen;
    if serial_len_calls.is_empty() {
        // Case 1: Struct has no fields.
        gen = quote! {
            impl crate::serde::wire::to_wire::ToWire for #name {
                #[inline]
                fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
                    Ok(())
                }
            
                #[inline]
                fn serial_length(&self) -> u16 { 0 }
            }
        };
    } else {
        // Case 2: Struct has 1+ fields.
        gen = quote! {
            impl crate::serde::wire::to_wire::ToWire for #name {
                #[inline]
                fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::serde::wire::compression_map::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
                    #to_wire_calls
                    Ok(())
                }
            
                #[inline]
                fn serial_length(&self) -> u16 {
                    #serial_len_calls
                }
            }
        };
    }
    gen.into()
}
