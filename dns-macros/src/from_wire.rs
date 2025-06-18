use quote::quote;
use syn::{Data, DataStruct, DeriveInput};

pub fn impl_from_wire_macro(ast: &DeriveInput) -> proc_macro::TokenStream {
    match &ast.data {
        Data::Struct(data) => impl_from_wire_struct_macro(data, ast),
        Data::Enum(_) => panic!("Enum not implemented"),
        Data::Union(_) => panic!("Union not implemented"),
    }
}

fn impl_from_wire_struct_macro(data: &DataStruct, ast: &DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;
    let field_name = data.fields.iter().map(|f| &f.ident);
    let field_type = data.fields.iter().map(|f| &f.ty);
    quote! {
        impl crate::serde::wire::from_wire::FromWire for #name {
            #[inline]
            fn from_wire_format<'a, 'b>(wire: &'b mut crate::serde::wire::read_wire::ReadWire<'a>) -> Result<Self, crate::serde::wire::read_wire::ReadWireError> where Self: Sized, 'a: 'b {
                Ok(Self {
                    #( #field_name: <#field_type>::from_wire_format(wire)? ),*
                })
            }
        }
    }.into()
}
