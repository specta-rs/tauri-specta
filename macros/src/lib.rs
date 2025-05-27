#![doc(
    // TODO: Tauri Specta logo
    html_logo_url = "https://github.com/oscartbeaumont/specta/raw/main/.github/logo-128.png",
    html_favicon_url = "https://github.com/oscartbeaumont/specta/raw/main/.github/logo-128.png"
)]

use heck::ToKebabCase;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, parse_quote, ConstParam, DeriveInput, GenericParam, Generics, Ident,
    LifetimeParam, LitStr, TypeParam, WhereClause,
};

use darling::FromDeriveInput;

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(tauri_specta), supports(struct_any, enum_any))]
struct EventOpts {
    ident: Ident,
    generics: Generics,

    #[darling(default)]
    event_name: Option<String>,
}

#[proc_macro_derive(Event, attributes(tauri_specta))]
pub fn derive_type(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let opts = match EventOpts::from_derive_input(&ast) {
        Ok(options) => options,
        Err(e) => {
            return e.write_errors().into();
        }
    };

    let crate_ref = quote!(tauri_specta);
    let ident = &opts.ident;
    let generics = &opts.generics;

    let name_str_value = match opts.event_name {
        Some(name_from_attr) => name_from_attr,
        None => ident.to_string().to_kebab_case(),
    };
    let name_lit = LitStr::new(&name_str_value, ident.span());

    let bounds = generics_with_ident_and_bounds_only(generics);
    let type_args = generics_with_ident_only(generics);
    let where_bound = add_type_to_where_clause(generics);

    quote! {
        #[automatically_derived]
        impl #bounds #crate_ref::Event for #ident #type_args #where_bound {
            const NAME: &'static str = #name_lit;
        }
    }
    .into()
}

fn generics_with_ident_and_bounds_only(generics: &Generics) -> Option<TokenStream> {
    (!generics.params.is_empty())
        .then(|| {
            use GenericParam::*;
            generics.params.iter().map(|param| match param {
                Type(TypeParam {
                    ident,
                    colon_token,
                    bounds,
                    ..
                }) => quote!(#ident #colon_token #bounds),
                Lifetime(LifetimeParam {
                    lifetime,
                    colon_token,
                    bounds,
                    ..
                }) => quote!(#lifetime #colon_token #bounds),
                Const(ConstParam {
                    const_token,
                    ident,
                    colon_token,
                    ty,
                    ..
                }) => quote!(#const_token #ident #colon_token #ty),
            })
        })
        .map(|gs| quote!(<#(#gs),*>))
}

fn generics_with_ident_only(generics: &Generics) -> Option<TokenStream> {
    (!generics.params.is_empty())
        .then(|| {
            use GenericParam::*;

            generics.params.iter().map(|param| match param {
                Type(TypeParam { ident, .. }) | Const(ConstParam { ident, .. }) => quote!(#ident),
                Lifetime(LifetimeParam { lifetime, .. }) => quote!(#lifetime),
            })
        })
        .map(|gs| quote!(<#(#gs),*>))
}

fn add_type_to_where_clause(generics: &Generics) -> Option<WhereClause> {
    let generic_types = generics
        .params
        .iter()
        .filter_map(|gp| match gp {
            GenericParam::Type(ty) => Some(ty.ident.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if generic_types.is_empty() {
        return generics.where_clause.clone();
    }
    match generics.where_clause {
        None => None,
        Some(ref w) => {
            let bounds = w.predicates.iter();
            Some(parse_quote! { where #(#bounds),* })
        }
    }
}
