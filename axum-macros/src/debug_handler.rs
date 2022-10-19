use crate::{
    attr_parsing::second,
    with_position::{Position, WithPosition},
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote, quote_spanned};
use syn::{parse_quote, spanned::Spanned, FnArg, ItemFn, Type};

use self::attr::Attrs;
use self::specializer::Specializer;

mod attr;
mod specializer;

pub(crate) fn expand(attr: Attrs, item_fn: ItemFn) -> TokenStream {
    let Attrs {
        body_ty,
        state_ty,
        with_tys,
    } = attr;
    let body_ty = body_ty
        .map(second)
        .unwrap_or_else(|| parse_quote!(axum::body::Body));
    let mut state_ty = state_ty.map(second);

    // these checks don't require the specializer, so we can generate code for them regardless
    // of whether we can successfully create one
    let check_extractor_count = check_extractor_count(&item_fn);
    let check_path_extractor = check_path_extractor(&item_fn);

    if state_ty.is_none() {
        state_ty = state_type_from_args(&item_fn);
    }
    let state_ty = state_ty.unwrap_or_else(|| syn::parse_quote!(()));

    // If the function is generic and an improper `with` statement was provided to the macro, we can't
    // reliably check its inputs or outputs. This will result in an error. We skip those checks to avoid
    // unhelpful additional compiler errors.
    let specializer_checks = match Specializer::new(with_tys, &item_fn) {
        Ok(specializer) => {
            let check_output_impls_into_response =
                check_output_impls_into_response(&item_fn, &specializer);
            let check_inputs_impls_from_request =
                check_inputs_impls_from_request(&item_fn, &body_ty, state_ty, &specializer);
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
        #check_path_extractor
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
    specializer: &Specializer,
) -> TokenStream {
    let takes_self = item_fn.sig.inputs.first().map_or(false, |arg| match arg {
        FnArg::Receiver(_) => true,
        FnArg::Typed(typed) => is_self_pat_type(typed),
    });

    WithPosition::new(item_fn.sig.inputs.iter())
        .enumerate()
        .map(|(arg_idx, arg)| {
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
            specializer
                .all_specializations_of_type(&ty)
                .enumerate()
                .map(|(specialization_idx, specialized_ty)| {
                    let check_fn = format_ident!(
                        "__axum_macros_check_{}_{}_{}_from_request_check",
                        item_fn.sig.ident,
                        arg_idx,
                        specialization_idx,
                        span = span,
                    );

                    let call_check_fn = format_ident!(
                        "__axum_macros_check_{}_{}_{}_from_request_call_check",
                        item_fn.sig.ident,
                        arg_idx,
                        specialization_idx,
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

                    let check_fn_generics = if must_impl_from_request_parts {
                        quote! {}
                    } else {
                        quote! { <M> }
                    };
        
                    let from_request_bound = if must_impl_from_request_parts {
                        quote! {
                            #specialized_ty: ::axum::extract::FromRequestParts<#state_ty> + Send
                        }
                    } else {
                        quote! {
                            #specialized_ty: ::axum::extract::FromRequest<#state_ty, #body_ty, M> + Send
                        }
                    };

                    quote_spanned! {span=>
                        #[allow(warnings)]
                        fn #check_fn #check_fn_generics()
                        where
                            #from_request_bound,
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
        })
        .collect::<TokenStream>()
}

/// generates specialized calls to the handler fn with each possible specialization. The
/// argument for each function param is a panic. This can be used to check that the handler output
/// value. The handler will have the proper receiver prepended to it if necessary (for instance Self::).
fn generate_mock_handler_calls<'a>(
    item_fn: &'a ItemFn,
    specializer: &'a Specializer,
) -> impl Iterator<Item = TokenStream> + 'a {
    let span = item_fn.span();
    let handler_name = &item_fn.sig.ident;
    specializer
        .all_specializations_as_turbofish()
        .map(move |turbofish| {
            // generic panics to use as the value for each argument to the handler function
            let args = item_fn.sig.inputs.iter().map(|_| {
                quote_spanned! {span=> panic!() }
            });
            if let Some(receiver) = self_receiver(item_fn) {
                quote_spanned! {span=>
                    #receiver #handler_name #turbofish (#(#args),*)
                }
            } else {
                // we have to repeat the item_fn in here as a dirty hack to
                // get around the situation where the handler fn is an associated
                // fn with no receiver -- we have no way to detect to use Self:: here
                quote_spanned! {span=>
                    {
                        #item_fn
                        #handler_name #turbofish (#(#args),*)
                    }
                }
            }
        })
}

fn check_output_impls_into_response(item_fn: &ItemFn, specializer: &Specializer) -> TokenStream {
    let return_ty_span = match &item_fn.sig.output {
        syn::ReturnType::Default => return quote! {},
        syn::ReturnType::Type(_, ty) => ty,
    }
    .span();
    generate_mock_handler_calls(item_fn, specializer)
        .enumerate()
        .map(|(specialization_idx, handler_call)| {
            let name = format_ident!(
                "__axum_macros_check_{}_{}_into_response",
                item_fn.sig.ident,
                specialization_idx
            );
            quote_spanned! {return_ty_span=>
                #[allow(warnings)]
                async fn #name() {
                    let value = #handler_call.await;

                    fn check<T>(_: T)
                        where T: ::axum::response::IntoResponse
                    {}

                    check(value);
                }
            }
        })
        .collect::<TokenStream>()
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
    generate_mock_handler_calls(item_fn, specializer)
        .enumerate()
        .map(|(specialization_idx, handler_call)| {
            let name = format_ident!(
                "__axum_macros_check_{}_{}_future",
                item_fn.sig.ident,
                specialization_idx
            );
            quote_spanned! {span=>
                #[allow(warnings)]
                fn #name() {
                    let future = #handler_call;
                    fn check<T>(_: T)
                        where T: ::std::future::Future + Send
                    {}
                    check(future);
                }
            }
        })
        .collect::<TokenStream>()
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
    let types = item_fn
        .sig
        .inputs
        .iter()
        .filter_map(|input| match input {
            FnArg::Receiver(_) => None,
            FnArg::Typed(pat_type) => Some(pat_type),
        })
        .map(|pat_type| &*pat_type.ty);
    crate::infer_state_type(types)
}

#[test]
fn ui() {
    crate::run_ui_tests("debug_handler");
}
