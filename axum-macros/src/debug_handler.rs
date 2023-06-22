use std::collections::HashSet;

use crate::{
    attr_parsing::{parse_assignment_attribute, second},
    with_position::{Position, WithPosition},
};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{parse::Parse, parse_quote, spanned::Spanned, FnArg, ItemFn, Token, Type};

pub(crate) fn expand(attr: Attrs, item_fn: ItemFn) -> TokenStream {
    let Attrs { body_ty, state_ty } = attr;

    let body_ty = body_ty
        .map(second)
        .unwrap_or_else(|| parse_quote!(axum::body::Body));

    let mut state_ty = state_ty.map(second);

    let check_extractor_count = check_extractor_count(&item_fn);
    let check_path_extractor = check_path_extractor(&item_fn);
    let check_output_impls_into_response = check_output_impls_into_response(&item_fn);

    // If the function is generic, we can't reliably check its inputs or whether the future it
    // returns is `Send`. Skip those checks to avoid unhelpful additional compiler errors.
    let check_inputs_and_future_send = if item_fn.sig.generics.params.is_empty() {
        let mut err = None;

        if state_ty.is_none() {
            let state_types_from_args = state_types_from_args(&item_fn);

            #[allow(clippy::comparison_chain)]
            if state_types_from_args.len() == 1 {
                state_ty = state_types_from_args.into_iter().next();
            } else if state_types_from_args.len() > 1 {
                err = Some(
                    syn::Error::new(
                        Span::call_site(),
                        "can't infer state type, please add set it explicitly, as in \
                         `#[debug_handler(state = MyStateType)]`",
                    )
                    .into_compile_error(),
                );
            }
        }

        err.unwrap_or_else(|| {
            let state_ty = state_ty.unwrap_or_else(|| syn::parse_quote!(()));

            let check_input_order = check_input_order(&item_fn);
            let check_future_send = check_future_send(&item_fn);

            if let Some(check_input_order) = check_input_order {
                quote! {
                    #check_input_order
                    #check_future_send
                }
            } else {
                let check_inputs_impls_from_request =
                    check_inputs_impls_from_request(&item_fn, &body_ty, state_ty);

                quote! {
                    #check_inputs_impls_from_request
                    #check_future_send
                }
            }
        })
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
    body_ty: Option<(kw::body, Type)>,
    state_ty: Option<(kw::state, Type)>,
}

impl Parse for Attrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut body_ty = None;
        let mut state_ty = None;

        while !input.is_empty() {
            let lh = input.lookahead1();
            if lh.peek(kw::body) {
                parse_assignment_attribute(input, &mut body_ty)?;
            } else if lh.peek(kw::state) {
                parse_assignment_attribute(input, &mut state_ty)?;
            } else {
                return Err(lh.error());
            }

            let _ = input.parse::<Token![,]>();
        }

        Ok(Self { body_ty, state_ty })
    }
}

fn check_extractor_count(item_fn: &ItemFn) -> Option<TokenStream> {
    let max_extractors = 16;
    if item_fn.sig.inputs.len() <= max_extractors {
        None
    } else {
        let error_message = format!(
            "Handlers cannot take more than {max_extractors} arguments. \
            Use `(a, b): (ExtractorA, ExtractorA)` to further nest extractors",
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

    WithPosition::new(item_fn.sig.inputs.iter())
        .enumerate()
        .map(|(idx, arg)| {
            let must_impl_from_request_parts = match &arg {
                Position::First(_) | Position::Middle(_) => true,
                Position::Last(_) | Position::Only(_) => false,
            };

            let arg = arg.into_inner();

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

            let consumes_request = request_consuming_type_name(&ty).is_some();

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

            let check_fn_generics = if must_impl_from_request_parts || consumes_request {
                quote! {}
            } else {
                quote! { <M> }
            };

            let from_request_bound = if must_impl_from_request_parts {
                quote_spanned! {span=>
                    #ty: ::axum::extract::FromRequestParts<#state_ty> + Send
                }
            } else if consumes_request {
                quote_spanned! {span=>
                    #ty: ::axum::extract::FromRequest<#state_ty, #body_ty> + Send
                }
            } else {
                quote_spanned! {span=>
                    #ty: ::axum::extract::FromRequest<#state_ty, #body_ty, M> + Send
                }
            };

            quote_spanned! {span=>
                #[allow(warnings)]
                #[allow(unreachable_code)]
                #[doc(hidden)]
                fn #check_fn #check_fn_generics()
                where
                    #from_request_bound,
                {}

                // we have to call the function to actually trigger a compile error
                // since the function is generic, just defining it is not enough
                #[allow(warnings)]
                #[allow(unreachable_code)]
                #[doc(hidden)]
                fn #call_check_fn()
                {
                    #call_check_fn_body
                }
            }
        })
        .collect::<TokenStream>()
}

fn check_input_order(item_fn: &ItemFn) -> Option<TokenStream> {
    let types_that_consume_the_request = item_fn
        .sig
        .inputs
        .iter()
        .enumerate()
        .filter_map(|(idx, arg)| {
            let ty = match arg {
                FnArg::Typed(pat_type) => &*pat_type.ty,
                FnArg::Receiver(_) => return None,
            };
            let type_name = request_consuming_type_name(ty)?;

            Some((idx, type_name, ty.span()))
        })
        .collect::<Vec<_>>();

    if types_that_consume_the_request.is_empty() {
        return None;
    };

    // exactly one type that consumes the request
    if types_that_consume_the_request.len() == 1 {
        // and that is not the last
        if types_that_consume_the_request[0].0 != item_fn.sig.inputs.len() - 1 {
            let (_idx, type_name, span) = &types_that_consume_the_request[0];
            let error = format!(
                "`{type_name}` consumes the request body and thus must be \
                the last argument to the handler function"
            );
            return Some(quote_spanned! {*span=>
                compile_error!(#error);
            });
        } else {
            return None;
        }
    }

    if types_that_consume_the_request.len() == 2 {
        let (_, first, _) = &types_that_consume_the_request[0];
        let (_, second, _) = &types_that_consume_the_request[1];
        let error = format!(
            "Can't have two extractors that consume the request body. \
            `{first}` and `{second}` both do that.",
        );
        let span = item_fn.sig.inputs.span();
        Some(quote_spanned! {span=>
            compile_error!(#error);
        })
    } else {
        let types = WithPosition::new(types_that_consume_the_request.into_iter())
            .map(|pos| match pos {
                Position::First((_, type_name, _)) | Position::Middle((_, type_name, _)) => {
                    format!("`{type_name}`, ")
                }
                Position::Last((_, type_name, _)) => format!("and `{type_name}`"),
                Position::Only(_) => unreachable!(),
            })
            .collect::<String>();

        let error = format!(
            "Can't have more than one extractor that consume the request body. \
            {types} all do that.",
        );
        let span = item_fn.sig.inputs.span();
        Some(quote_spanned! {span=>
            compile_error!(#error);
        })
    }
}

fn request_consuming_type_name(ty: &Type) -> Option<&'static str> {
    let path = match ty {
        Type::Path(type_path) => &type_path.path,
        _ => return None,
    };

    let ident = match path.segments.last() {
        Some(path_segment) => &path_segment.ident,
        None => return None,
    };

    let type_name = match &*ident.to_string() {
        "Json" => "Json<_>",
        "BodyStream" => "BodyStream",
        "RawBody" => "RawBody<_>",
        "RawForm" => "RawForm",
        "Multipart" => "Multipart",
        "Protobuf" => "Protobuf",
        "JsonLines" => "JsonLines<_>",
        "Form" => "Form<_>",
        "Request" => "Request<_>",
        "Bytes" => "Bytes",
        "String" => "String",
        "Parts" => "Parts",
        _ => return None,
    };

    Some(type_name)
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
            #[allow(unreachable_code)]
            #[doc(hidden)]
            async fn #make_value_name() -> #ty {
                #declare_inputs
                #block
            }
        }
    } else {
        quote_spanned! {span=>
            #[allow(warnings)]
            #[allow(unreachable_code)]
            #[doc(hidden)]
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
            #[allow(unreachable_code)]
            #[doc(hidden)]
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
            #[allow(unreachable_code)]
            #[doc(hidden)]
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
                    item_fn.sig.fn_token,
                    "Handlers must be `async fn`s",
                )
                .into_compile_error();
            }
            syn::ReturnType::Type(_, ty) => ty,
        };
    }

    let span = item_fn.sig.ident.span();

    let handler_name = &item_fn.sig.ident;

    let args = item_fn.sig.inputs.iter().map(|_| {
        quote_spanned! {span=> panic!() }
    });

    let name = format_ident!("__axum_macros_check_{}_future", item_fn.sig.ident);

    let do_check = quote! {
        fn check<T>(_: T)
            where T: ::std::future::Future + Send
        {}
        check(future);
    };

    if let Some(receiver) = self_receiver(item_fn) {
        quote! {
            #[allow(warnings)]
            #[allow(unreachable_code)]
            #[doc(hidden)]
            fn #name() {
                let future = #receiver #handler_name(#(#args),*);
                #do_check
            }
        }
    } else {
        quote! {
            #[allow(warnings)]
            #[allow(unreachable_code)]
            #[doc(hidden)]
            fn #name() {
                #item_fn
                let future = #handler_name(#(#args),*);
                #do_check
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
fn state_types_from_args(item_fn: &ItemFn) -> HashSet<Type> {
    let types = item_fn
        .sig
        .inputs
        .iter()
        .filter_map(|input| match input {
            FnArg::Receiver(_) => None,
            FnArg::Typed(pat_type) => Some(pat_type),
        })
        .map(|pat_type| &*pat_type.ty);
    crate::infer_state_types(types).collect()
}

#[test]
fn ui() {
    crate::run_ui_tests("debug_handler");
}
