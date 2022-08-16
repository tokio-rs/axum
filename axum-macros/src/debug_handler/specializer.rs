use std::collections::{HashMap, HashSet};

use proc_macro2::TokenStream;
use syn::{
    parse_quote,
    visit::{self, Visit},
    visit_mut::{self, VisitMut},
};

use quote::quote;

struct GenericFinder<'a> {
    found_idents: HashSet<&'a syn::Ident>,
    generic_param_idents: &'a HashSet<syn::Ident>,
}

impl<'ast, 'a> Visit<'ast> for GenericFinder<'a> {
    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        if let Some(a) = self.generic_param_idents.get(ident) {
            self.found_idents.insert(a);
        }
        // Delegate to the default impl to visit nested expressions.
        visit::visit_ident(self, ident);
    }
}

/// find which generic_param_idents are present in typ
fn find_generic_args_in_type<'a, 'b>(
    typ: &'a syn::Type,
    generic_param_idents: &'b HashSet<syn::Ident>,
) -> HashSet<&'b syn::Ident> {
    let mut finder = GenericFinder {
        found_idents: HashSet::new(),
        generic_param_idents: generic_param_idents,
    };
    finder.visit_type(typ);
    finder.found_idents
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
    generic_param_set: HashSet<syn::Ident>,
    specializations: HashMap<syn::Ident, Vec<syn::Type>>,
}

impl Specializer {
    pub(crate) fn new(
        generic_params: Vec<syn::Ident>,
        specializations: HashMap<syn::Ident, Vec<syn::Type>>,
    ) -> Self {
        let generic_param_set = HashSet::from_iter(generic_params.iter().cloned());
        Specializer {
            generic_params,
            generic_param_set,
            specializations,
        }
    }

    fn compute_specializations<'a>(
        &'a self,
        typ: &'a syn::Type,
    ) -> Option<impl Iterator<Item = syn::Type> + 'a> {
        // to avoid generating the cross product of all generic params on
        // the function we first iterate over the type to find which generic
        // params are involed, then we generate the cross product
        // using the specializations of those params only.
        let generic_params = find_generic_args_in_type(typ, &self.generic_param_set);
        if generic_params.is_empty() {
            return None;
        }
        if generic_params.len() != 1 {
            // TODO
            unimplemented!();
        }
        // assume the expression contains the identity of a single generic expression
        let generic_param_ident = *generic_params.iter().next().unwrap();
        let specializations = self.specializations.get(generic_param_ident).unwrap();

        Some(specializations.into_iter().map(move |specialized_typ| {
            let param_specs = HashMap::from([(generic_param_ident, specialized_typ)]);
            let mut specializer = TypeSpecializer {
                specializations: &param_specs,
            };
            let mut new_typ = typ.clone();
            specializer.visit_type_mut(&mut new_typ);
            new_typ
        }))
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
    pub(crate) fn all_specializations(&self, typ: &syn::Type) -> Vec<syn::Type> {
        self.compute_specializations(typ)
            .map(|i| i.collect())
            .unwrap_or_else(|| vec![typ.clone()])
    }

    /// Behaves the same way as `all_specializations` except only returns a single
    /// default specialization.
    pub(crate) fn specialize_default(&self, typ: &syn::Type) -> syn::Type {
        self.compute_specializations(typ)
            .and_then(|mut i| i.next())
            .unwrap_or_else(|| typ.clone())
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
                .expect("there should be at least one specialization per generic type param")
        });
        quote! { ::<#(#default_handler_specializations),*> }
    }
}
