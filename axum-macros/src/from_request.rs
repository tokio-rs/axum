use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

pub(crate) fn expand(item: syn::ItemStruct) -> syn::Result<TokenStream> {
    let syn::ItemStruct {
        attrs: _,
        ident,
        generics,
        fields,
        semi_token: _,
        vis: _,
        struct_token: _,
    } = item;

    let extract_fields = match fields {
        syn::Fields::Named(fields) => extract_fields(fields.named.iter()),
        syn::Fields::Unnamed(fields) => extract_fields(fields.unnamed.iter()),
        syn::Fields::Unit => Default::default(),
    };

    if !generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            generics,
            "`#[derive(FromRequest)] doesn't support generics",
        ));
    }

    if let Some(where_clause) = generics.where_clause {
        return Err(syn::Error::new_spanned(
            where_clause,
            "`#[derive(FromRequest)] doesn't support generics",
        ));
    }

    Ok(quote! {
        #[::axum::async_trait]
        impl<B> ::axum::extract::FromRequest<B> for #ident
        where
            B: ::axum::body::HttpBody + ::std::marker::Send + 'static,
            B::Data: ::std::marker::Send,
            B::Error: ::std::convert::Into<::axum::BoxError>,
        {
            type Rejection = ::axum::response::Response;

            async fn from_request(
                req: &mut ::axum::extract::RequestParts<B>,
            ) -> ::std::result::Result<Self, Self::Rejection> {
                ::std::result::Result::Ok(Self {
                    #(#extract_fields)*
                })
            }
        }
    })
}

fn extract_fields<'a, I>(fields: I) -> Vec<TokenStream>
where
    I: Iterator<Item = &'a syn::Field>,
{
    fields
        .enumerate()
        .map(|(index, field)| {
            let member = if let Some(ident) = &field.ident {
                quote! { #ident }
            } else {
                let member = syn::Member::Unnamed(syn::Index {
                    index: index as u32,
                    span: field.span(),
                });
                quote! { #member }
            };

            let span = field.ty.span();

            quote_spanned! {span=>
                #member: {
                    ::axum::extract::FromRequest::from_request(req)
                        .await
                        .map_err(::axum::response::IntoResponse::into_response)?
                },
            }
        })
        .collect()
}
