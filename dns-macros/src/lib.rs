extern crate proc_macro;
use proc_macro::TokenStream;

mod to_wire;
mod from_wire;
use to_wire::impl_to_wire_macro;
use from_wire::impl_from_wire_macro;

mod from_tokenized_rdata;
use from_tokenized_rdata::impl_from_tokenized_rdata_macro;

mod to_presentation;
use to_presentation::impl_to_presentation_macro;

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

#[proc_macro_derive(FromTokenizedRData)]
pub fn derive_from_tokenized_rdata(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_from_tokenized_rdata_macro(&ast)
}

#[proc_macro_derive(ToPresentation)]
pub fn derive_to_presentation(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_to_presentation_macro(&ast)
}

#[proc_macro_derive(RTypeCode)]
pub fn derive_rtype_code(input: TokenStream) -> TokenStream {
    // Construct a representation of Rust code as a syntax tree
    // that we can manipulate
    let ast = syn::parse(input).unwrap();

    // Build the trait implementation
    impl_rtype_code_macro(&ast)
}
