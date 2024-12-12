use proc_macro;
use syn::DeriveInput;
use quote::quote;

pub fn impl_rdata_macro(ast: &DeriveInput) -> proc_macro::TokenStream {
    let name = &ast.ident;

    quote! {
        impl #name {
            pub const RTYPE: crate::resource_record::rtype::RType = crate::resource_record::rtype::RType::#name;
        }

        impl crate::resource_record::resource_record::RData for #name {
            #[inline]
            fn get_rtype(&self) -> crate::resource_record::rtype::RType { Self::RTYPE }
        }
    }.into()
}
