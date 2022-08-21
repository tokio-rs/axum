use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use std::collections::HashSet;
use syn::{parse::Parse, spanned::Spanned, FnArg, ItemFn, Token, Type};

pub(crate) fn expand(mut attr: Attrs, item_fn: ItemFn) -> TokenStream {
    let check_extractor_count = check_extractor_count(&item_fn);
    let check_path_extractor = check_path_extractor(&item_fn);
    let check_output_impls_into_response = check_output_impls_into_response(&item_fn);

    // If the function is generic, we can't reliably check its inputs or whether the future it
    // returns is `Send`. Skip those checks to avoid unhelpful additional compiler errors.
    let check_inputs_and_future_send = if item_fn.sig.generics.params.is_empty() {
        if attr.state_ty.is_none() {
            attr.state_ty = state_type_from_args(&item_fn);
        }

        let state_ty = attr.state_ty.unwrap_or_else(|| syn::parse_quote!(()));

        let check_inputs_impls_from_request =
            check_inputs_impls_from_request(&item_fn, &attr.body_ty, state_ty);
        let check_future_send = check_future_send(&item_fn);

        quote! {
            #check_inputs_impls_from_request
            #check_future_send
        }
    } else {
        syn::Error::new_spanned(
            &item_fn.sig.generics,
            "`#[axum_macros::debug_handler]` doesn't support generic functions",
        )
        .into_compile_error()
    };

    quote! {
        #item_fn
        #check_extractor_count
        #check_path_extractor
        #check_output_impls_into_response
        #check_inputs_and_future_send
    }
}

mod kw {
    syn::custom_keyword!(body);
    syn::custom_keyword!(state);
}

pub(crate) struct Attrs {
    body_ty: Type,
    state_ty: Option<Type>,
}

impl Parse for Attrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut body_ty = None;
        let mut state_ty = None;

        while !input.is_empty() {
            let lh = input.lookahead1();

            if lh.peek(kw::body) {
                let kw = input.parse::<kw::body>()?;
                if body_ty.is_some() {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "`body` specified more than once",
                    ));
                }
                input.parse::<Token![=]>()?;
                body_ty = Some(input.parse()?);
            } else if lh.peek(kw::state) {
                let kw = input.parse::<kw::state>()?;
                if state_ty.is_some() {
                    return Err(syn::Error::new_spanned(
                        kw,
                        "`state` specified more than once",
                    ));
                }
                input.parse::<Token![=]>()?;
                state_ty = Some(input.parse()?);
            } else {
                return Err(lh.error());
            }

            let _ = input.parse::<Token![,]>();
        }

        let body_ty = body_ty.unwrap_or_else(|| syn::parse_quote!(axum::body::Body));

        Ok(Self { body_ty, state_ty })
    }
}

fn check_extractor_count(item_fn: &ItemFn) -> Option<TokenStream> {
    let max_extractors = 16;
    if item_fn.sig.inputs.len() <= max_extractors {
        None
    } else {
        let error_message = format!(
            "Handlers cannot take more than {} arguments. \
            Use `(a, b): (ExtractorA, ExtractorA)` to further nest extractors",
            max_extractors,
        );
        let error = syn::Error::new_spanned(&item_fn.sig.inputs, error_message).to_compile_error();
        Some(error)
    }
}

fn extractor_idents(item_fn: &ItemFn) -> impl Iterator<Item = (usize, &syn::FnArg, &syn::Ident)> {
    item_fn
        .sig
        .inputs
        .iter()
        .enumerate()
        .filter_map(|(idx, fn_arg)| match fn_arg {
            FnArg::Receiver(_) => None,
            FnArg::Typed(pat_type) => {
                if let Type::Path(type_path) = &*pat_type.ty {
                    type_path
                        .path
                        .segments
                        .last()
                        .map(|segment| (idx, fn_arg, &segment.ident))
                } else {
                    None
                }
            }
        })
}

fn check_path_extractor(item_fn: &ItemFn) -> TokenStream {
    let path_extractors = extractor_idents(item_fn)
        .filter(|(_, _, ident)| *ident == "Path")
        .collect::<Vec<_>>();

    if path_extractors.len() > 1 {
        path_extractors
            .into_iter()
            .map(|(_, arg, _)| {
                syn::Error::new_spanned(
                    arg,
                    "Multiple parameters must be extracted with a tuple \
                    `Path<(_, _)>` or a struct `Path<YourParams>`, not by applying \
                    multiple `Path<_>` extractors",
                )
                .to_compile_error()
            })
            .collect()
    } else {
        quote! {}
    }
}

fn is_self_pat_type(typed: &syn::PatType) -> bool {
    let ident = if let syn::Pat::Ident(ident) = &*typed.pat {
        &ident.ident
    } else {
        return false;
    };

    ident == "self"
}

fn check_inputs_impls_from_request(
    item_fn: &ItemFn,
    body_ty: &Type,
    state_ty: Type,
) -> TokenStream {
    let takes_self = item_fn.sig.inputs.first().map_or(false, |arg| match arg {
        FnArg::Receiver(_) => true,
        FnArg::Typed(typed) => is_self_pat_type(typed),
    });

    item_fn
        .sig
        .inputs
        .iter()
        .enumerate()
        .map(|(idx, arg)| {
            let (span, ty) = match arg {
                FnArg::Receiver(receiver) => {
                    if receiver.reference.is_some() {
                        return syn::Error::new_spanned(
                            receiver,
                            "Handlers must only take owned values",
                        )
                        .into_compile_error();
                    }

                    let span = receiver.span();
                    (span, syn::parse_quote!(Self))
                }
                FnArg::Typed(typed) => {
                    let ty = &typed.ty;
                    let span = ty.span();

                    if is_self_pat_type(typed) {
                        (span, syn::parse_quote!(Self))
                    } else {
                        (span, ty.clone())
                    }
                }
            };

            let check_fn = format_ident!(
                "__axum_macros_check_{}_{}_from_request_check",
                item_fn.sig.ident,
                idx,
                span = span,
            );

            let call_check_fn = format_ident!(
                "__axum_macros_check_{}_{}_from_request_call_check",
                item_fn.sig.ident,
                idx,
                span = span,
            );

            let call_check_fn_body = if takes_self {
                quote_spanned! {span=>
                    Self::#check_fn();
                }
            } else {
                quote_spanned! {span=>
                    #check_fn();
                }
            };

            quote_spanned! {span=>
                #[allow(warnings)]
                fn #check_fn<M>()
                where
                    #ty: ::axum::extract::FromRequest<#state_ty, #body_ty, M> + Send,
                {}

                // we have to call the function to actually trigger a compile error
                // since the function is generic, just defining it is not enough
                #[allow(warnings)]
                fn #call_check_fn()
                {
                    #call_check_fn_body
                }
            }
        })
        .collect::<TokenStream>()
}

fn check_output_impls_into_response(item_fn: &ItemFn) -> TokenStream {
    let ty = match &item_fn.sig.output {
        syn::ReturnType::Default => return quote! {},
        syn::ReturnType::Type(_, ty) => ty,
    };
    let span = ty.span();

    let declare_inputs = item_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Receiver(_) => None,
            FnArg::Typed(pat_ty) => {
                let pat = &pat_ty.pat;
                let ty = &pat_ty.ty;
                Some(quote! {
                    let #pat: #ty = panic!();
                })
            }
        })
        .collect::<TokenStream>();

    let block = &item_fn.block;

    let make_value_name = format_ident!(
        "__axum_macros_check_{}_into_response_make_value",
        item_fn.sig.ident
    );

    let make = if item_fn.sig.asyncness.is_some() {
        quote_spanned! {span=>
            #[allow(warnings)]
            async fn #make_value_name() -> #ty {
                #declare_inputs
                #block
            }
        }
    } else {
        quote_spanned! {span=>
            #[allow(warnings)]
            fn #make_value_name() -> #ty {
                #declare_inputs
                #block
            }
        }
    };

    let name = format_ident!("__axum_macros_check_{}_into_response", item_fn.sig.ident);

    if let Some(receiver) = self_receiver(item_fn) {
        quote_spanned! {span=>
            #make

            #[allow(warnings)]
            async fn #name() {
                let value = #receiver #make_value_name().await;
                fn check<T>(_: T)
                    where T: ::axum::response::IntoResponse
                {}
                check(value);
            }
        }
    } else {
        quote_spanned! {span=>
            #[allow(warnings)]
            async fn #name() {
                #make

                let value = #make_value_name().await;

                fn check<T>(_: T)
                where T: ::axum::response::IntoResponse
                {}

                check(value);
            }
        }
    }
}

fn check_future_send(item_fn: &ItemFn) -> TokenStream {
    if item_fn.sig.asyncness.is_none() {
        match &item_fn.sig.output {
            syn::ReturnType::Default => {
                return syn::Error::new_spanned(
                    &item_fn.sig.fn_token,
                    "Handlers must be `async fn`s",
                )
                .into_compile_error();
            }
            syn::ReturnType::Type(_, ty) => ty,
        };
    }

    let span = item_fn.span();

    let handler_name = &item_fn.sig.ident;

    let args = item_fn.sig.inputs.iter().map(|_| {
        quote_spanned! {span=> panic!() }
    });

    let name = format_ident!("__axum_macros_check_{}_future", item_fn.sig.ident);

    if let Some(receiver) = self_receiver(item_fn) {
        quote_spanned! {span=>
            #[allow(warnings)]
            fn #name() {
                let future = #receiver #handler_name(#(#args),*);
                fn check<T>(_: T)
                    where T: ::std::future::Future + Send
                {}
                check(future);
            }
        }
    } else {
        quote_spanned! {span=>
            #[allow(warnings)]
            fn #name() {
                #item_fn

                let future = #handler_name(#(#args),*);
                fn check<T>(_: T)
                    where T: ::std::future::Future + Send
                {}
                check(future);
            }
        }
    }
}

fn self_receiver(item_fn: &ItemFn) -> Option<TokenStream> {
    let takes_self = item_fn.sig.inputs.iter().any(|arg| match arg {
        FnArg::Receiver(_) => true,
        FnArg::Typed(typed) => is_self_pat_type(typed),
    });

    if takes_self {
        return Some(quote! { Self:: });
    }

    if let syn::ReturnType::Type(_, ty) = &item_fn.sig.output {
        if let syn::Type::Path(path) = &**ty {
            let segments = &path.path.segments;
            if segments.len() == 1 {
                if let Some(last) = segments.last() {
                    match &last.arguments {
                        syn::PathArguments::None if last.ident == "Self" => {
                            return Some(quote! { Self:: });
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    None
}

/// Given a signature like
///
/// ```skip
/// #[debug_handler]
/// async fn handler(
///     _: axum::extract::State<AppState>,
///     _: State<AppState>,
/// ) {}
/// ```
///
/// This will extract `AppState`.
///
/// Returns `None` if there are no `State` args or multiple of different types.
fn state_type_from_args(item_fn: &ItemFn) -> Option<Type> {
    let state_inputs = item_fn
        .sig
        .inputs
        .iter()
        .filter_map(|input| match input {
            FnArg::Receiver(_) => None,
            FnArg::Typed(pat_type) => Some(pat_type),
        })
        .map(|pat_type| &pat_type.ty)
        .filter_map(|ty| {
            if let Type::Path(path) = &**ty {
                Some(&path.path)
            } else {
                None
            }
        })
        .filter_map(|path| {
            if let Some(last_segment) = path.segments.last() {
                if last_segment.ident != "State" {
                    return None;
                }

                match &last_segment.arguments {
                    syn::PathArguments::AngleBracketed(args) if args.args.len() == 1 => {
                        Some(args.args.first().unwrap())
                    }
                    _ => None,
                }
            } else {
                None
            }
        })
        .filter_map(|generic_arg| {
            if let syn::GenericArgument::Type(ty) = generic_arg {
                Some(ty)
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();

    if state_inputs.len() == 1 {
        state_inputs.iter().next().map(|&ty| ty.clone())
    } else {
        None
    }
}

#[test]
fn ui() {
    #[rustversion::stable]
    fn go() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/debug_handler/fail/*.rs");
        t.pass("tests/debug_handler/pass/*.rs");
    }

    #[rustversion::not(stable)]
    fn go() {}

    go();
}
