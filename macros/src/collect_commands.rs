use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Path, Token,
};

pub struct Input {
    paths: Punctuated<Path, Token![,]>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            paths: Punctuated::parse_terminated(input)?,
        })
    }
}

pub fn proc_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let Input { paths } = parse_macro_input!(input as Input);

    let tauri_paths = paths.iter().map(|p| {
        let Path {
            leading_colon,
            segments,
        } = p;

        let segments = segments.iter().map(|s| &s.ident);

        quote!(#leading_colon #(#segments)::*)
    });

    let body = quote! {(
        ::specta::function::collect_functions![#paths],
        ::tauri::generate_handler![#(#tauri_paths),*],
    )};

    quote! {{
        #body
    }}
    .into()
}
