use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::{spanned::Spanned, FnArg, ItemFn, Type};

use self::attr::Attrs;
use self::specializer::Specializer;

mod attr;
mod specializer;

pub(crate) fn expand(attr: Attrs, item_fn: ItemFn) -> TokenStream {
    // TODO error on const generics and liftetimes
    // TODO error on not all generic params specified using with

    // these checks don't require the specializer, so we can generate code for them regardless
    // of whether we can successfully create one
    let check_extractor_count = check_extractor_count(&item_fn);
    let check_request_last_extractor = check_request_last_extractor(&item_fn);
    let check_path_extractor = check_path_extractor(&item_fn);
    let check_multiple_body_extractors = check_multiple_body_extractors(&item_fn);

    // If the function is generic and and improper `with` statement was provided to the macro, we can't
    // reliably check its inputs or outputs. This will result in an error. We skip those checks to avoid
    // unhelpful additional compiler errors.
    let specializer_checks = match Specializer::new(&attr, &item_fn) {
        Ok(specializer) => {
            let check_output_impls_into_response =
                check_output_impls_into_response(&item_fn, &specializer);
            let check_inputs_impls_from_request =
                check_inputs_impls_from_request(&item_fn, attr.body_ty(), &specializer);
            let check_future_send = check_future_send(&item_fn, &specializer);
            quote! {
                #check_output_impls_into_response
                #check_inputs_impls_from_request
                #check_future_send
            }
        }
        Err(err) => err.into_compile_error(),
    };

    quote! {
        #item_fn
        #check_extractor_count
        #check_request_last_extractor
        #check_path_extractor
        #check_multiple_body_extractors
        #specializer_checks
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

fn check_request_last_extractor(item_fn: &ItemFn) -> Option<TokenStream> {
    let request_extractor_ident =
        extractor_idents(item_fn).find(|(_, _, ident)| *ident == "Request");

    if let Some((idx, fn_arg, _)) = request_extractor_ident {
        if idx != item_fn.sig.inputs.len() - 1 {
            return Some(
                syn::Error::new_spanned(fn_arg, "`Request` extractor should always be last")
                    .to_compile_error(),
            );
        }
    }

    None
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

fn check_multiple_body_extractors(item_fn: &ItemFn) -> TokenStream {
    let body_extractors = extractor_idents(item_fn)
        .filter(|(_, _, ident)| {
            *ident == "String"
                || *ident == "Bytes"
                || *ident == "Json"
                || *ident == "RawBody"
                || *ident == "BodyStream"
                || *ident == "Multipart"
                || *ident == "Request"
        })
        .collect::<Vec<_>>();

    if body_extractors.len() > 1 {
        body_extractors
            .into_iter()
            .map(|(_, arg, _)| {
                syn::Error::new_spanned(arg, "Only one body extractor can be applied")
                    .to_compile_error()
            })
            .collect()
    } else {
        quote! {}
    }
}

fn check_inputs_impls_from_request(
    item_fn: &ItemFn,
    body_ty: &Type,
    specializer: &Specializer,
) -> TokenStream {
    item_fn
        .sig
        .inputs
        .iter()
        .enumerate()
        .map(|(arg_idx, arg)| {
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
                    (span, ty.clone())
                }
            };

            specializer
                .all_specializations(&ty)
                .iter()
                .enumerate()
                .map(|(specialization_idx, specialized_ty)| {
                    let name = format_ident!(
                        "__axum_macros_check_{}_{}_{}_from_request",
                        item_fn.sig.ident,
                        arg_idx,
                        specialization_idx,
                    );
                    quote_spanned! {span=>
                        #[allow(warnings)]
                        fn #name()
                        where
                            #specialized_ty: ::axum::extract::FromRequest<#body_ty> + Send,
                        {}
                    }
                })
                .collect::<TokenStream>()
        })
        .collect::<TokenStream>()
}

fn check_output_impls_into_response(item_fn: &ItemFn, specializer: &Specializer) -> TokenStream {
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
                let ty = specializer.specialize_default(&pat_ty.ty);
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

fn check_future_send(item_fn: &ItemFn, specializer: &Specializer) -> TokenStream {
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

    let default_handler_specialization = specializer.make_turbofish_with_default_specializations();

    // TODO generics test for receiver
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

                let future = #handler_name #default_handler_specialization (#(#args),*);
                fn check<T>(_: T)
                    where T: ::std::future::Future + Send
                {}
                check(future);
            }
        }
    }
}

fn self_receiver(item_fn: &ItemFn) -> Option<TokenStream> {
    let takes_self = item_fn
        .sig
        .inputs
        .iter()
        .any(|arg| matches!(arg, syn::FnArg::Receiver(_)));
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
