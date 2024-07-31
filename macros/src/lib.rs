use heck::ToKebabCase;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Event, attributes(tauri_specta))]
pub fn derive_type(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let crate_ref = quote!(tauri_specta);

    let DeriveInput { ident, .. } = parse_macro_input!(input);

    let name = ident.to_string().to_kebab_case();

    quote! {
        impl #crate_ref::Event for #ident {
            const NAME: &'static str = #name;
        }
    }
    .into()
}
