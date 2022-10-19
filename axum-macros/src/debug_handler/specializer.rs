use std::collections::{HashMap, HashSet};

use itertools::Itertools;
use proc_macro2::TokenStream;
use syn::{
    visit::{self, Visit},
    visit_mut::{self, VisitMut},
    GenericParam, ItemFn,
};

use quote::quote;

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
    generic_params: Vec<syn::Ident>,
    specializations: HashMap<syn::Ident, Vec<syn::Type>>,
}

impl Specializer {
    pub(crate) fn new(
        with_tys: Option<SpecializationsAttr>,
        item_fn: &ItemFn,
    ) -> Result<Self, syn::Error> {
        let specializations = with_tys.map(|f| f.specializations).unwrap_or_default();

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
}
