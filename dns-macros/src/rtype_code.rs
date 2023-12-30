use proc_macro;
use syn::{DeriveInput, Data, DataStruct};
use quote::quote;

pub fn impl_rtype_code_macro(ast: &DeriveInput) -> proc_macro::TokenStream {
    match &ast.data {
        Data::Struct(data) => impl_rtype_code_struct_macro(data, ast),
        Data::Enum(_) => panic!("Enum not implemented"),
        Data::Union(_) => panic!("Union not implemented"),
    }
}

fn impl_rtype_code_struct_macro(_data: &DataStruct, ast: &DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;

    quote! {
        impl #name {
            pub const RTYPE: crate::resource_record::rtype::RType = crate::resource_record::rtype::RType::#name;
        }

        impl crate::resource_record::rtype::RTypeCode for #name {
            #[inline]
            fn rtype(&self) -> crate::resource_record::rtype::RType { Self::RTYPE }
        }
    }.into()
}
