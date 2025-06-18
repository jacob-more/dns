use quote::quote;
use syn::{Data, DataStruct, DeriveInput};

pub fn impl_to_wire_macro(ast: &DeriveInput) -> proc_macro::TokenStream {
    match &ast.data {
        Data::Struct(data) => impl_to_wire_struct_macro(data, ast),
        Data::Enum(_) => panic!("Enum not implemented"),
        Data::Union(_) => panic!("Union not implemented"),
    }
}

fn impl_to_wire_struct_macro(data: &DataStruct, ast: &DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;
    let field_name_to_wire = data.fields.iter().map(|f| &f.ident);
    let field_name_serial_len = data.fields.iter().map(|f| &f.ident);

    quote! {
        impl crate::serde::wire::to_wire::ToWire for #name {
            #[inline]
            fn to_wire_format<'a, 'b>(&self, wire: &'b mut crate::serde::wire::write_wire::WriteWire<'a>, compression: &mut Option<crate::types::c_domain_name::CompressionMap>) -> Result<(), crate::serde::wire::write_wire::WriteWireError> where 'a: 'b {
                #( self.#field_name_to_wire.to_wire_format(wire, compression)?; )*
                Ok(())
            }

            #[inline]
            fn serial_length(&self) -> u16 {
                0 #( + self.#field_name_serial_len.serial_length() )*
            }
        }
    }.into()
}
