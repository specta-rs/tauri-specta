#![doc(
    html_logo_url = "https://github.com/specta-rs/tauri-specta/raw/main/.github/logo.png",
    html_favicon_url = "https://github.com/specta-rs/tauri-specta/raw/main/.github/logo.png"
)]

use heck::ToKebabCase;
use proc_macro2::Span;
use quote::{quote, quote_spanned};
use syn::{
    DeriveInput, Generics, Ident, LitStr, Path, PathArguments, Token, ext::IdentExt,
    parse_macro_input, parse_quote, punctuated::Punctuated,
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

    // Keep collector locals separate from unqualified command paths at the call site.
    let types = Ident::new("types", Span::mixed_site());
    let collected = Ident::new("collected", Span::mixed_site());
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
                #types,
            )
        };
        inferences.push(quote! {
            #collected.commands.push(#call);
        });
    }

    quote! {
        ::tauri_specta::internal::command(
            ::tauri::generate_handler![#(#handlers),*],
            |#[allow(unused_variables)] #types: &mut ::specta::Types| {
                #[allow(unused_mut)]
                let mut #collected = ::tauri_specta::CollectedCommands::default();
                #(#inferences)*
                #collected
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
    let mut generics = opts.generics;

    let name_str_value = match opts.event_name {
        Some(name_from_attr) => name_from_attr,
        None => ident.unraw().to_string().to_kebab_case(),
    };
    let name_lit = LitStr::new(&name_str_value, ident.span());

    if !generics.params.is_empty() {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(Self: #crate_ref::internal::Type + 'static));
    }
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    quote! {
        #[automatically_derived]
        impl #impl_generics #crate_ref::Event for #ident #type_generics #where_clause {
            const NAME: &'static str = #name_lit;
        }
    }
    .into()
}
