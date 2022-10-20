use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use proc_macro2::{Span, TokenStream};
use syn::{
    spanned::Spanned,
    visit::{self, Visit},
    visit_mut::{self, VisitMut},
    FnArg, GenericParam, ItemFn, Type,
};

use quote::{format_ident, quote, quote_spanned};

use crate::with_position::{Position, WithPosition};

use super::attr::SpecializationsAttr;

struct GenericFinder<'a> {
    found_idents: HashSet<&'a syn::Ident>,
    generic_param_set: HashSet<&'a syn::Ident>,
}

impl<'ast, 'a> Visit<'ast> for GenericFinder<'a> {
    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        if let Some(a) = self.generic_param_set.get(ident) {
            self.found_idents.insert(a);
        }
        // Delegate to the default impl to visit nested expressions.
        visit::visit_ident(self, ident);
    }
}

struct TypeSpecializer<'a> {
    specializations: &'a HashMap<&'a syn::Ident, &'a syn::Type>,
}

impl<'a> VisitMut for TypeSpecializer<'a> {
    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        match ty {
            syn::Type::Path(ty_path) => {
                if ty_path.path.segments.len() == 1 {
                    let ident = &ty_path.path.segments[0].ident;
                    if let Some(specialized) = self.specializations.get(ident) {
                        *ty = (*specialized).clone();
                        return; // don't recuresively visit substituted values
                    }
                }
            }
            _ => (),
        };
        visit_mut::visit_type_mut(self, ty);
    }
}

pub(crate) struct Specializer {
    item_fn: ItemFn,
    generic_params: Vec<syn::Ident>,
    specializations: HashMap<syn::Ident, Vec<syn::Type>>,
    body_ty: Type,
    state_ty: Type,
}

impl Specializer {
    pub(crate) fn new(
        with_tys: Option<SpecializationsAttr>,
        item_fn: ItemFn,
        body_ty: Type,
        state_ty_param: Option<Type>,
    ) -> Result<Self, syn::Error> {
        let specializations = with_tys.map(|f| f.specializations).unwrap_or_default();

        let generic_params = item_fn
            .sig
            .generics
            .params
            .iter()
            .enumerate()
            .map(|(_idx, param)| match param {
                    GenericParam::Type(t) => {
                        if specializations.contains_key(&t.ident) {  
                            Ok(t.ident.clone())
                        } else {
                            Err(                                
                                syn::Error::new_spanned(
                                    param,
                                    "Generic param is missing a specialization in `#[axum_macros::debug_handler]`. Specify each generic param at least once using `#[debug_handler(with(T = ConcreteType))]`.",
                                )
                            )
                        }                    
                    },
                    _ => Err(syn::Error::new_spanned(
                        param,
                        "Only type params are supported by `#[axum_macros::debug_handler]`.",
                    )),
                }
            )
            .collect::<Result<Vec<_>, _>>()?;

        let state_ty = state_type_from_param(state_ty_param, &item_fn)?;

        Ok(Specializer {
            item_fn,
            generic_params,
            specializations,
            body_ty,
            state_ty,
        })
    }

    /// Returns iterator that moves through all possible specializations of generic parameters.
    ///
    /// Each specialization is a vector of concrete types. The order of the types in the vector
    /// corresponds to the order of the generic parameters in the function signature. If the function
    /// has no generic parameters, the iterator will return a single empty vector.
    ///
    /// If filter is specified, the cross product is only taken over the specified generic params. Otherwise
    /// it is taken over all generic params. Specifying a generic param in filter that is not in self.generic_params
    /// is UB.
    fn all_specializations<'a>(
        &'a self,
        filter: Option<HashSet<&syn::Ident>>,
    ) -> Box<dyn Iterator<Item = Vec<&'a syn::Type>> + 'a> {
        let generic_params = match filter {
            Some(filter) => self
                .generic_params
                .iter()
                .filter(|param| filter.contains(param))
                .collect::<Vec<_>>(),
            None => self.generic_params.iter().collect::<Vec<_>>(),
        };
        if generic_params.is_empty() {
            Box::new(std::iter::once(vec![]))
        } else {
            Box::new(
                generic_params
                    .into_iter()
                    // SAFETY: we can unwrap here due the invariant in the constructor
                    .map(|p| {
                        self.specializations
                            .get(p)
                            .expect("should be specialization per param")
                    })
                    .multi_cartesian_product(),
            )
        }
    }

    /// Like `all_specializations_of_fn` but produces turbofishes of each specialization, rather than modified ItemFn.
    pub(crate) fn all_specializations_as_turbofish<'a>(
        &'a self,
    ) -> impl Iterator<Item = TokenStream> + 'a {
        self.all_specializations(None)
            .map(move |specializations| quote! { ::<#(#specializations),*> })
    }

    /// Return vector of generic param identities found in the given type.
    ///
    /// Each param will be present in the returned vec at most once, and will be in order
    /// of appearance from self.generic_params
    fn find_generic_params<'a>(&'a self, typ: &'_ syn::Type) -> HashSet<&'a syn::Ident> {
        let generic_param_set = HashSet::from_iter(self.generic_params.iter());
        let mut finder = GenericFinder {
            found_idents: HashSet::new(),
            generic_param_set,
        };
        finder.visit_type(typ);
        finder.found_idents
    }

    /// For a given type, parameterized by the generics of item_fn, return all possible
    /// concrete types given the specified debug specializations. If the type does not
    /// contain any generic parameters, returns a vector with a single item which is a clone
    /// of the original type.
    ///
    /// Since some types may be generic over several params (eg `Foo<X, Y>`)
    /// the number of returned specializations is the size of the cross product of all
    /// specializations over the generic params that are present in the passed in typ.
    ///
    /// Substitution is done deeply, that is, for a given set of specializations the syntax tree
    /// for `typ` is searched deeply, recursively replacing each ocurrence of the generic parameters
    /// which ensures that all substitutions are made even in very complex cases such
    /// as `<<T as Trait>::Foo as some_crate::OtherTrait<U>>::Bar`.
    ///
    /// This function will only search for generic params named in `generic_params`, everything
    /// else will be assumed to be a concrete type.    
    ///
    /// Example:
    /// Assume a handler with two generic arguments `T` and `U` and the  debug specializations
    /// (T = u32, T = String, U = i32).
    ///     compute_all_specializations(Path<T>) would yield [Path<u32>, Path<String>]
    ///     compute_all_specializations(Path<U>) would yield [Path<i32>]
    ///     compute_all_specializations(Foo<T, U>) would yield [Foo<u32, i32>, Foo<String, i32>]
    ///     compute_all_specializations(U) would yield [i32]
    ///     compute_all_specializations(String) would yield [String]
    ///     
    pub(crate) fn all_specializations_of_type<'a>(
        &'a self,
        typ: &'a syn::Type,
    ) -> impl Iterator<Item = syn::Type> + 'a {
        let ty_params = self.find_generic_params(typ);
        self.all_specializations(Some(ty_params.clone()))
            .map(move |specializations| {
                let param_specs: HashMap<&syn::Ident, &syn::Type> = HashMap::from_iter(
                    std::iter::zip(ty_params.iter().map(|f| *f), specializations),
                );
                let mut specializer = TypeSpecializer {
                    specializations: &param_specs,
                };
                let mut new_typ = typ.clone();
                specializer.visit_type_mut(&mut new_typ);
                new_typ
            })
    }

    /// Generates specialized calls to the handler fn with each possible specialization. The
    /// argument for each function param is a panic. This can be used to check that the handler output
    /// value. The handler will have the proper receiver prepended to it if necessary (for instance Self::).
    fn generate_mock_handler_calls(&self) -> impl Iterator<Item = TokenStream> + '_ {
        let span = self.item_fn.span();
        let handler_name = &self.item_fn.sig.ident;
        self.all_specializations_as_turbofish()
            .map(move |turbofish| {
                // generic panics to use as the value for each argument to the handler function
                let args = self.item_fn.sig.inputs.iter().map(|_| {
                    quote_spanned! {span=> panic!() }
                });
                if let Some(receiver) = self.generate_self_receiver() {
                    quote_spanned! {span=>
                        #receiver #handler_name #turbofish (#(#args),*)
                    }
                } else {
                    let item_fn = &self.item_fn;
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

    /// Generate the receiver for the handler function, if necessary.
    fn generate_self_receiver(&self) -> Option<TokenStream> {
        let takes_self = self.item_fn.sig.inputs.iter().any(|arg| match arg {
            FnArg::Receiver(_) => true,
            FnArg::Typed(typed) => is_self_pat_type(typed),
        });

        if takes_self {
            return Some(quote! { Self:: });
        }

        if let syn::ReturnType::Type(_, ty) = &self.item_fn.sig.output {
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

    /// Generates a series of of functions that will only compile if the output
    /// of the handler function implements `::axum::response::IntoResponse`.
    pub(crate) fn generate_check_output_impls_into_response(&self) -> TokenStream {
        let return_ty_span = match &self.item_fn.sig.output {
            syn::ReturnType::Default => return quote! {},
            syn::ReturnType::Type(_, ty) => ty,
        }
        .span();
        self.generate_mock_handler_calls()
            .enumerate()
            .map(|(specialization_idx, handler_call)| {
                let name = format_ident!(
                    "__axum_macros_check_{}_{}_into_response",
                    self.item_fn.sig.ident,
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
            .collect()
    }

    /// Generates a series of of functions that will only compile if the output
    /// of the handler function is `::std::future::Future + Send`.
    pub(crate) fn generate_check_output_future_send(&self) -> TokenStream {
        if self.item_fn.sig.asyncness.is_none() {
            match &self.item_fn.sig.output {
                syn::ReturnType::Default => {
                    return syn::Error::new_spanned(
                        &self.item_fn.sig.fn_token,
                        "Handlers must be `async fn`s",
                    )
                    .into_compile_error();
                }
                syn::ReturnType::Type(_, ty) => ty,
            };
        }
        let span = self.item_fn.span();
        self.generate_mock_handler_calls()
            .enumerate()
            .map(|(specialization_idx, handler_call)| {
                let name = format_ident!(
                    "__axum_macros_check_{}_{}_future",
                    self.item_fn.sig.ident,
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
            .collect()
    }

    /// Generates a series of of functions that will only compile if the arguments to item_fn
    /// implement `::axum::extract::FromRequest` or `::axum::extract::FromRequestParts`.
    pub(crate) fn generate_check_inputs_impl_from_request(&self) -> TokenStream {
        let takes_self = self
            .item_fn
            .sig
            .inputs
            .first()
            .map_or(false, |arg| match arg {
                FnArg::Receiver(_) => true,
                FnArg::Typed(typed) => is_self_pat_type(typed),
            });

        WithPosition::new(self.item_fn.sig.inputs.iter())
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
                self
                    .all_specializations_of_type(&ty)
                    .enumerate()
                    .map(|(specialization_idx, specialized_ty)| {
                        let check_fn = format_ident!(
                            "__axum_macros_check_{}_{}_{}_from_request_check",
                            self.item_fn.sig.ident,
                            arg_idx,
                            specialization_idx,
                            span = span,
                        );
    
                        let call_check_fn = format_ident!(
                            "__axum_macros_check_{}_{}_{}_from_request_call_check",
                            self.item_fn.sig.ident,
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
                        
                        let state_ty = &self.state_ty;
                        let body_ty = &self.body_ty;
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
                            fn #call_check_fn() {
                                #call_check_fn_body
                            }
                        }
                    })
                    .collect::<TokenStream>()
            })
            .collect()
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

/// This tries to extract or infer the state type given the item_fn and the
/// param supplied to the macro.
fn state_type_from_param(
    state_ty_param: Option<Type>,
    item_fn: &ItemFn,
) -> Result<Type, syn::Error> {
    match state_ty_param {
        Some(ty) => Ok(ty),
        None => {
            let state_types_from_args = state_types_from_args(item_fn);
            let r = if state_types_from_args.len() == 1 {
                Ok(state_types_from_args.into_iter().next())
            } else if state_types_from_args.len() > 1 {
                Err(syn::Error::new(
                    Span::call_site(),
                    "can't infer state type, please add set it explicitly, as in \
                        `#[debug_handler(state = MyStateType)]`",
                ))
            } else {
                Ok(None)
            };
            r.map(|t| t.unwrap_or_else(|| syn::parse_quote!(())))
        }
    }
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
