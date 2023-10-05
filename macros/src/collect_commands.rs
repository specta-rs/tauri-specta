use proc_macro2::Ident;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Path, Token,
};

pub struct Input {
    type_map: Option<Ident>,
    paths: Punctuated<Path, Token![,]>,
}

mod kw {
    syn::custom_keyword!(type_map);
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            type_map: {
                if input.peek(kw::type_map) && input.peek2(Token![:]) {
                    input.parse::<kw::type_map>()?;
                    input.parse::<Token![:]>()?;
                    Some(input.parse()?)
                } else {
                    None
                }
            },
            paths: Punctuated::parse_terminated(input)?,
        })
    }
}

pub fn proc_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let Input { type_map, paths } = parse_macro_input!(input as Input);

    let tauri_paths = paths.iter().map(|p| {
        let Path {
            leading_colon,
            segments,
        } = p;

        let segments = segments.iter().map(|s| &s.ident);

        quote!(#leading_colon #(#segments)::*)
    });

    let type_map = type_map
        .map(|i| quote!(#i))
        .unwrap_or_else(|| quote!(::specta::TypeMap::new()));

    let body = quote! {(
        ::specta::collect_functions![type_map; #paths],
        ::tauri::generate_handler![#(#tauri_paths),*],
    )};

    quote! {{
        let mut type_map = #type_map;

        #body
    }}
    .into()
}
