#![doc(
    // TODO: Tauri Specta logo
    html_logo_url = "https://github.com/oscartbeaumont/specta/raw/main/.github/logo-128.png",
    html_favicon_url = "https://github.com/oscartbeaumont/specta/raw/main/.github/logo-128.png"
)]

use std::str::FromStr;

use heck::ToKebabCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse_macro_input, parse_quote, ConstParam, DeriveInput, FnArg, GenericParam, Generics,
    ImplItem, Item, LifetimeParam, Pat, ReturnType, Token, TypeParam, Visibility, WhereClause,
};

#[proc_macro_derive(Event, attributes(tauri_specta))]
pub fn derive_type(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let crate_ref = quote!(tauri_specta);

    let DeriveInput {
        ident, generics, ..
    } = parse_macro_input!(input);

    let name = ident.to_string().to_kebab_case();
    let bounds = generics_with_ident_and_bounds_only(&generics);
    let type_args = generics_with_ident_only(&generics);
    let where_bound = add_type_to_where_clause(&generics);

    quote! {
        #[automatically_derived]
        impl #bounds #crate_ref::Event for #ident #type_args #where_bound {
            const NAME: &'static str = #name;
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

#[proc_macro_attribute]
pub fn class(
    _attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let crate_ref = quote!(tauri_specta);
    let item = syn::parse_macro_input!(input as syn::ItemImpl);

    // TODO: Proper syn error
    assert_eq!(
        item.generics.params.len(),
        0,
        "generics not supported with #[specta::class]"
    );

    let ident = &item.self_ty;
    let ident_str = ident.to_token_stream().to_string();

    let methods = item.items.iter().filter_map(|item| match item {
        ImplItem::Fn(function) => {
            let crate_ref = quote!(specta);
            let fn_ident = &function.sig.ident;

            // TODO
            // assert!(
            //     function.sig.unsafety != Token![unsafe],
            //     "unsafe functions are not supported with #[specta::class]"
            // );
            // assert!(
            //     function.sig.abi.is_some(),
            //     "ABI declarations are not supported with #[specta::class]"
            // );
            assert!(
                function.sig.generics.params.is_empty(),
                "generics not supported with #[specta::class]"
            );
            assert!(
                function.sig.generics.where_clause.is_none(),
                "where clauses not supported with #[specta::class]"
            );

            if function
                .sig
                .inputs
                .iter()
                .find(|i| match i {
                    FnArg::Receiver(receiver) => receiver.reference.is_some(),
                    FnArg::Typed(_) => false,
                })
                .is_some()
            {
                todo!("`&self` and `&mut self` are not supported with #[specta::class]")
            }

            let visibility = &function.vis;
            let maybe_macro_export = match &visibility {
                Visibility::Public(_) => {
                    quote!(#[macro_export])
                }
                _ => Default::default(),
            };

            let function_name = &function.sig.ident;
            let function_name_str = unraw(&function_name.to_string()).to_string();
            let function_asyncness = match function.sig.asyncness {
                Some(_) => true,
                None => false,
            };

            let mut arg_names = Vec::new();
            for input in function.sig.inputs.iter() {
                let arg = match input {
                    FnArg::Receiver(_) => quote!(東京),
                    FnArg::Typed(arg) => match &*arg.pat {
                        Pat::Ident(ident) => ident.ident.to_token_stream(),
                        Pat::Macro(m) => m.mac.tokens.to_token_stream(),
                        Pat::Struct(s) => s.path.to_token_stream(),
                        Pat::Slice(s) => s.attrs[0].to_token_stream(),
                        Pat::Tuple(s) => s.elems[0].to_token_stream(),
                        _ => {
                            // return Err(syn::Error::new_spanned(
                            //     input,
                            //     "functions with `#[specta]` must take named arguments",
                            // ))
                            todo!("functions with `#[specta]` must take named arguments");
                        }
                    },
                };

                let mut s = arg.to_string();

                let s = if s.starts_with("r#") {
                    s.split_off(2)
                } else {
                    s
                };

                arg_names.push(TokenStream::from_str(&s).unwrap());
            }

            let arg_signatures = function.sig.inputs.iter().map(|_| quote!(_));

            // TODO: Support attributes
            // let mut attrs = parse_attrs(&function.attrs)?;
            // let common = crate::r#type::attr::CommonAttr::from_attrs(&mut attrs)?;

            // let deprecated = common.deprecated_as_tokens(&crate_ref);
            // let docs = common.doc;
            let deprecated = quote!(None);
            let docs = "";

            let no_return_type = match function.sig.output {
                syn::ReturnType::Default => true,
                syn::ReturnType::Type(_, _) => false,
            };

            let wrapper_ident = format_ident!(
                "__{}__{}",
                ident.into_token_stream().to_string(),
                &function.sig.ident
            );

            Some(quote! {
                // TODO: Can this all just be `fn_datatype` on the wrapper function?
                // specta::function::fn_datatype!(#wrapper_ident)(types)

                // TODO: This is the same as the inside of `specta::specta`
                // TODO: This is not really intended to be semver-stable
                #crate_ref::internal::get_fn_datatype(
                    // #ident::#fn_ident as fn(#(#arg_signatures),*) -> _, // TODO: Causing `self`
                    #wrapper_ident as fn(#(#arg_signatures),*) -> _,
                    #function_asyncness,
                    #function_name_str.into(),
                    types,
                    &[#(stringify!(#arg_names).into()),* ],
                    std::borrow::Cow::Borrowed(#docs),
                    #deprecated,
                    #no_return_type,
                )
            })
        }
        _ => None,
    });

    // TODO: I wonder we could PR something like this to Tauri or would that be too much???
    let tauri_command_wrappers = item.items.iter().filter_map(|item| match item {
        ImplItem::Fn(function) => {
            let attrs = &function.attrs;
            let vis = &function.vis;
            let asyncness = &function.sig.asyncness;
            let wait = &function
                .sig
                .asyncness
                .as_ref()
                .map(|_| quote!(.await))
                .unwrap_or_default();
            let original_ident = &function.sig.ident;
            let wrapper_ident = format_ident!(
                "__{}__{}",
                ident.into_token_stream().to_string(),
                &function.sig.ident
            );

            let (args_def, args): (Vec<_>, Vec<_>) = function
                .sig
                .inputs
                .iter()
                .map(|v| {
                    let arg = match v {
                        FnArg::Receiver(_) => quote!(東京),
                        FnArg::Typed(arg) => match &*arg.pat {
                            Pat::Ident(ident) => ident.ident.to_token_stream(),
                            Pat::Macro(m) => m.mac.tokens.to_token_stream(),
                            Pat::Struct(s) => s.path.to_token_stream(),
                            Pat::Slice(s) => s.attrs[0].to_token_stream(),
                            Pat::Tuple(s) => s.elems[0].to_token_stream(),
                            _ => {
                                // return Err(syn::Error::new_spanned(
                                //     input,
                                //     "functions with `#[specta]` must take named arguments",
                                // ))
                                todo!("functions with `#[specta]` must take named arguments");
                            }
                        },
                    };

                    let mut s = arg.to_string();

                    let s = if s.starts_with("r#") {
                        s.split_off(2)
                    } else {
                        s
                    };

                    let arg_ident = TokenStream::from_str(&s).unwrap();

                    (
                        match v {
                            FnArg::Receiver(_) => quote!(#arg_ident: #ident),
                            FnArg::Typed(ty) => {
                                let ty = &ty.ty;
                                quote!(#arg_ident: #ty)
                            }
                        },
                        quote!(#arg_ident),
                    )
                })
                .unzip();
            let ret_type = &function.sig.output;

            Some(quote! {
                #[tauri::command]
                #[specta::specta]
                #(#attrs)*
                #vis #asyncness fn #wrapper_ident(#(#args_def),*) #ret_type {
                    #ident::#original_ident(#(#args),*) #wait
                }
            })
        }
        _ => None,
    });

    quote! {
        #item

        impl #crate_ref::Class for #ident {
            fn collect(types: &mut specta::TypeCollection) -> #crate_ref::ClassDefinition {
                <Self as specta::Type>::inline(types, specta::Generics::Definition);

                #crate_ref::ClassDefinition {
                    ident: #ident_str,
                    ndt: types.remove(<Self as specta::NamedType>::sid()).expect("we register the type above!"),
                    methods: vec![#(#methods),*],
                }
            }
        }

        #(#tauri_command_wrappers)*
    }
    .into()
}

fn unraw(s: &str) -> &str {
    if s.starts_with("r#") {
        s.split_at(2).1
    } else {
        s.as_ref()
    }
}
