use quote::ToTokens;
use syn::{
    parse::{Parse, ParseStream},
    Token,
};

pub(crate) fn parse_parenthesized_attribute<K, T>(
    input: ParseStream<'_>,
    out: &mut Option<(K, T)>,
) -> syn::Result<()>
where
    K: Parse + ToTokens,
    T: Parse,
{
    let kw = input.parse()?;

    let content;
    syn::parenthesized!(content in input);
    let inner = content.parse()?;

    if out.is_some() {
        let kw_name = std::any::type_name::<K>().split("::").last().unwrap();
        let msg = format!("`{kw_name}` specified more than once");
        return Err(syn::Error::new_spanned(kw, msg));
    }

    *out = Some((kw, inner));

    Ok(())
}

pub(crate) fn parse_assignment_attribute<K, T>(
    input: ParseStream<'_>,
    out: &mut Option<(K, T)>,
) -> syn::Result<()>
where
    K: Parse + ToTokens,
    T: Parse,
{
    let kw = input.parse()?;
    input.parse::<Token![=]>()?;
    let inner = input.parse()?;

    if out.is_some() {
        let kw_name = std::any::type_name::<K>().split("::").last().unwrap();
        let msg = format!("`{kw_name}` specified more than once");
        return Err(syn::Error::new_spanned(kw, msg));
    }

    *out = Some((kw, inner));

    Ok(())
}

pub(crate) trait Combine: Sized {
    fn combine(self, other: Self) -> syn::Result<Self>;
}

pub(crate) fn parse_attrs<T>(ident: &str, attrs: &[syn::Attribute]) -> syn::Result<T>
where
    T: Combine + Default + Parse,
{
    attrs
        .iter()
        .filter(|attr| attr.meta.path().is_ident(ident))
        .map(|attr| attr.parse_args::<T>())
        .try_fold(T::default(), |out, next| out.combine(next?))
}

pub(crate) fn combine_attribute<K, T>(a: &mut Option<(K, T)>, b: Option<(K, T)>) -> syn::Result<()>
where
    K: ToTokens,
{
    if let Some((kw, inner)) = b {
        if a.is_some() {
            let kw_name = std::any::type_name::<K>().split("::").last().unwrap();
            let msg = format!("`{kw_name}` specified more than once");
            return Err(syn::Error::new_spanned(kw, msg));
        }
        *a = Some((kw, inner));
    }
    Ok(())
}

pub(crate) fn combine_unary_attribute<K>(a: &mut Option<K>, b: Option<K>) -> syn::Result<()>
where
    K: ToTokens,
{
    if let Some(kw) = b {
        if a.is_some() {
            let kw_name = std::any::type_name::<K>().split("::").last().unwrap();
            let msg = format!("`{kw_name}` specified more than once");
            return Err(syn::Error::new_spanned(kw, msg));
        }
        *a = Some(kw);
    }
    Ok(())
}

pub(crate) fn second<T, K>(tuple: (T, K)) -> K {
    tuple.1
}
