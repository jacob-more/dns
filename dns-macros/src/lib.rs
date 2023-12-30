extern crate proc_macro;
use proc_macro::TokenStream;

mod to_wire;
mod from_wire;
use to_wire::impl_to_wire_macro;
use from_wire::impl_from_wire_macro;

mod from_tokenized_record;
use from_tokenized_record::impl_from_tokenized_record_macro;

mod rtype_code;
use rtype_code::impl_rtype_code_macro;

#[proc_macro_derive(ToWire)]
pub fn derive_to_wire(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_to_wire_macro(&ast)
}

#[proc_macro_derive(FromWire)]
pub fn derive_from_wire(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_from_wire_macro(&ast)
}

#[proc_macro_derive(FromTokenizedRecord)]
pub fn derive_from_tokenized_record(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_from_tokenized_record_macro(&ast)
}

#[proc_macro_derive(RTypeCode)]
pub fn derive_rtype_code(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_rtype_code_macro(&ast)
}
