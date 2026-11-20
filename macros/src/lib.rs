#![doc(
    html_logo_url = "https://github.com/specta-rs/tauri-specta/raw/main/.github/logo.png",
    html_favicon_url = "https://github.com/specta-rs/tauri-specta/raw/main/.github/logo.png"
)]

use heck::ToKebabCase;
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{
    ConstParam, DeriveInput, GenericParam, Generics, Ident, LifetimeParam, LitStr, Path,
    PathArguments, Token, TypeParam, WhereClause, ext::IdentExt, parse_macro_input, parse_quote,
    punctuated::Punctuated,
};

use darling::FromDeriveInput;

/// Collect commands and their types.
///
/// This is a combination of Tauri's `generate_handler!` and Tauri's command
/// introspection, returning `tauri_specta::Commands` for `Builder::commands`.
///
/// Commands only need `#[tauri::command]`; `#[specta::specta]` is not required. The
/// argument and result types come from the command's Rust signature, so anything the
/// frontend sends must implement `specta::Type`.
///
/// A `#[deprecated]` command is referenced by value to infer its signature, which the
/// `deprecated` lint reports at the call site. Add `#[allow(deprecated)]` where you call
/// this macro if your crate denies that lint.
#[proc_macro]
pub fn collect_commands(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let commands = parse_macro_input!(input with Punctuated::<Path, Token![,]>::parse_terminated);

    let mut handlers = Vec::new();
    let mut inferences = Vec::new();

    for command in &commands {
        let stripped = strip_path_generics(command);
        let accessor = command
            .segments
            .last()
            .map(|segment| segment.ident.unraw().to_string())
            .unwrap_or_default();

        handlers.push(quote!(#stripped));
        let span = command
            .segments
            .last()
            .map_or_else(Span::call_site, |segment| segment.ident.span());
        let call = quote_spanned! {span=>
            ::tauri_specta::internal::infer_command(
                #command,
                ::tauri::command_metadata!(#stripped),
                #accessor,
                types,
            )
        };
        inferences.push(quote! {
            collected.commands.push(#call);
        });
    }

    quote! {
        ::tauri_specta::internal::command(
            ::tauri::generate_handler![#(#handlers),*],
            {
                #[allow(unused_variables, unused_mut)]
                fn export(
                    types: &mut ::specta::Types,
                ) -> ::tauri_specta::CollectedCommands {
                    let mut collected = ::tauri_specta::CollectedCommands::default();
                    #(#inferences)*
                    collected
                }

                export
            },
        )
    }
    .into()
}

/// Remove `::<...>` so the path can be passed to Tauri's handler and metadata macros,
/// neither of which accepts generics.
fn strip_path_generics(path: &Path) -> Path {
    let mut path = path.clone();
    for segment in &mut path.segments {
        segment.arguments = PathArguments::None;
    }
    path
}

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
