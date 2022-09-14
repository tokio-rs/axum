use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use proc_macro2::TokenStream;
use syn::{
    parse_quote,
    visit::{self, Visit},
    visit_mut::{self, VisitMut},
    GenericParam, ItemFn,
};

use quote::quote;

use super::attr::Attrs;

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
    fn visit_ident_mut(&mut self, ident: &mut syn::Ident) {
        if let Some(specialized) = self.specializations.get(ident) {
            *ident = parse_quote!(#specialized);
        }
        // Delegate to the default impl to visit nested expressions.
        visit_mut::visit_ident_mut(self, ident);
    }
}
pub(crate) struct Specializer {
    generic_params: Vec<syn::Ident>,
    specializations: HashMap<syn::Ident, Vec<syn::Type>>,
}

impl Specializer {
    pub(crate) fn new(attr: &Attrs, item_fn: &ItemFn) -> Result<Self, syn::Error> {
        let specializations = attr
            .specializations()
            .map(|s| s.clone())
            .unwrap_or_default();

        let generic_params = item_fn
            .sig
            .generics
            .params
            .iter()
            .enumerate()
            .map(|(_idx, param)| match param {
                GenericParam::Type(t) => Ok(t.ident.clone()),
                _ => Err(syn::Error::new_spanned(
                    param,
                    "Only type params are supported by `#[axum_macros::debug_handler]`.",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;

        for param in &generic_params {
            if !specializations.contains_key(param) {
                return Err(
                    syn::Error::new_spanned(
                        param,
                        "Generic param is missing a specialization in `#[axum_macros::debug_handler]`. Specify each generic param at least once using the 'with' attribute to support debugging generic functions.",
                    )
                );
            }
        }

        Ok(Specializer {
            generic_params,
            specializations,
        })
    }

    /// Produce a specialized version of the function for each possible specialization of the generic
    /// params The number of specialized functions produced is equal to the number of possible combinations
    /// of the specializations (ie the cartesian product of the param sets passed to this Specializer).
    ///
    /// If no generic params, this will produce a single specialization of the function, equal to the one passed in.
    pub(crate) fn all_specializations_of_fn<'a>(
        &'a self,
        item_fn: ItemFn,
    ) -> Box<dyn Iterator<Item = syn::ItemFn> + 'a> {
        if self.generic_params.is_empty() {
            return Box::new(std::iter::once(item_fn));
        }
        Box::new(
            self.generic_params
                .iter()
                // SAFETY: we can unwrap here due the invariant in the constructor
                .map(|p| self.specializations.get(p).unwrap())
                .multi_cartesian_product()
                .map(move |specialized_values| {
                    let param_specs: HashMap<&syn::Ident, &syn::Type> = HashMap::from_iter(
                        std::iter::zip(self.generic_params.iter(), specialized_values),
                    );
                    let mut type_specializer = TypeSpecializer {
                        specializations: &param_specs,
                    };
                    let mut item_fn = item_fn.clone();
                    type_specializer.visit_item_fn_mut(&mut item_fn);
                    item_fn
                }),
        )
    }

    /// Return vector of generic param identities found in the given type.
    ///
    /// Each param will be present in the returned vec at most once, and will be in order
    /// of appearance from self.generic_params
    fn find_generic_params<'a>(&'a self, typ: &'_ syn::Type) -> Vec<&'a syn::Ident> {
        let generic_param_set = HashSet::from_iter(self.generic_params.iter());
        let mut finder = GenericFinder {
            found_idents: HashSet::new(),
            generic_param_set,
        };
        finder.visit_type(typ);
        self.generic_params
            .iter()
            .filter(|i| finder.found_idents.contains(i))
            .collect()
    }

    fn compute_specializations_of_type<'a>(
        &'a self,
        typ: &'a syn::Type,
    ) -> Option<impl Iterator<Item = syn::Type> + 'a> {
        // to avoid generating the cross product of all generic params on
        // the function we first iterate over the type to find which generic
        // params are involed, then we generate the cross product
        // using the specializations of those params only.
        let ty_params = self.find_generic_params(typ);
        if ty_params.is_empty() {
            return None;
        }

        let ty_param_specializations = ty_params.iter().map(|param| {
            // safety: we can unwrap here due to the initializer invariant
            // that all generic params have at least one specialization
            self.specializations
                .get(param)
                .expect("there should be at least one specialization per generic type param")
                .iter()
        });

        Some(
            ty_param_specializations
                .multi_cartesian_product()
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
                }),
        )
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
    pub(crate) fn all_specializations_of_type(&self, typ: &syn::Type) -> Vec<syn::Type> {
        self.compute_specializations_of_type(typ)
            .map(|i| i.collect())
            .unwrap_or_else(|| vec![typ.clone()])
    }

    /// Create a token stream with the default generic arg specializations
    /// for the handler, in turbofish syntax.
    ///
    /// For instance, for `fn handler<T, U>(t: Path<T>, u: Path<U>)` with `#[debug_handler(with(T = String, T = u64, U = i32))]`
    /// this would return `::<String, i32>`. The choice of the "default" specialization for each variable type
    /// is made by choosing the first specialization to appear in the `with` attribute.
    ///
    /// For a non-generic function, this returns the empty turbofish `::<>`.
    pub(crate) fn make_turbofish_with_default_specializations(&self) -> TokenStream {
        let default_handler_specializations = self.generic_params.iter().map(|f| {
            self.specializations
                .get(f)
                .and_then(|v| v.first())
                // safety: we can unwrap here due to the initializer invariant
                // that all generic params have at least one specialization
                .expect("there should be at least one specialization per generic type param")
        });
        quote! { ::<#(#default_handler_specializations),*> }
    }
}
