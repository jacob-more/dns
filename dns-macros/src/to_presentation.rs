use quote::quote;
use syn::{Data, DataStruct, DeriveInput};

pub fn impl_to_presentation_macro(ast: &DeriveInput) -> proc_macro::TokenStream {
    match &ast.data {
        Data::Struct(data) => impl_to_presentation_struct_macro(data, ast),
        Data::Enum(_) => panic!("Enum not implemented"),
        Data::Union(_) => panic!("Union not implemented"),
    }
}

fn impl_to_presentation_struct_macro(
    data: &DataStruct,
    ast: &DeriveInput,
) -> proc_macro::TokenStream {
    let name = &ast.ident;
    let field_name = data.fields.iter().map(|f| &f.ident);
    let field_type = data.fields.iter().map(|f| &f.ty);

    quote! {
        impl crate::serde::presentation::to_presentation::ToPresentation for #name {
            #[inline]
            fn to_presentation_format(&self, out_buffer: &mut Vec<String>) {
                #( <#field_type as crate::serde::presentation::to_presentation::ToPresentation>::to_presentation_format(&self.#field_name, out_buffer) );*
            }
        }
    }.into()
}
