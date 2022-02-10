use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::borrow::Cow;
use syn::{spanned::Spanned, ItemStruct, LitStr};

pub(crate) fn expand(item_struct: ItemStruct) -> syn::Result<TokenStream> {
    let ItemStruct {
        attrs,
        vis,
        struct_token: _,
        ident,
        generics,
        fields,
        semi_token: _,
    } = &item_struct;

    if !generics.params.is_empty() || generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            generics,
            "`#[derive(Path)]` doesn't support generics",
        ));
    }

    let Attrs { route } = parse_attrs(attrs)?;

    let route_method = route_method(&route, vis, fields);
    let from_request_impl = from_request_impl(ident, fields);

    Ok(quote! {
        #[automatically_derived]
        impl #ident {
            #vis const ROUTE: &'static str = #route;

            #route_method
        }

        #from_request_impl
    })
}

#[derive(Debug)]
struct Attrs {
    route: String,
}

fn parse_attrs(attrs: &[syn::Attribute]) -> syn::Result<Attrs> {
    let mut route = None::<String>;

    for attr in attrs {
        if attr.path.is_ident("route") {
            route = Some(attr.parse_args::<LitStr>()?.value());
        }
    }

    Ok(Attrs {
        route: route.ok_or_else(|| {
            syn::Error::new(Span::call_site(), "missing `#[route(\"...\")]` attribute")
        })?,
    })
}

fn route_method(route: &str, vis: &syn::Visibility, fields: &syn::Fields) -> TokenStream {
    let format_str = path_into_format_str(route, matches!(fields, syn::Fields::Unnamed(_)));

    match fields {
        syn::Fields::Named(fields) => {
            let set_placeholders = fields.named.iter().map(|field| {
                let ident = &field.ident;
                quote! { #ident = self.#ident }
            });

            quote! {
                #vis fn route(&self) -> String {
                    format!(#format_str, #(#set_placeholders,)*)
                }
            }
        }
        syn::Fields::Unnamed(fields) => {
            let set_placeholders = fields.unnamed.iter().enumerate().map(|(index, field)| {
                let field = syn::Member::Unnamed(syn::Index {
                    index: index as _,
                    span: field.span(),
                });
                quote! { self.#field }
            });

            quote! {
                #vis fn route(&self) -> String {
                    format!(#format_str, #(#set_placeholders,)*)
                }
            }
        }
        syn::Fields::Unit => quote! {
            #vis fn route(&self) -> &'static str {
                #route
            }
        },
    }
}

fn path_into_format_str(route: &str, index_placeholders: bool) -> String {
    let mut index = 0;

    route
        .split('/')
        .map(|segment| {
            if let Some(capture) = segment.strip_prefix(':') {
                if index_placeholders {
                    let segment = Cow::Owned(format!("{{{}}}", index));
                    index += 1;
                    segment
                } else {
                    Cow::Owned(format!("{{{}}}", capture))
                }
            } else {
                Cow::Borrowed(segment)
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn from_request_impl(ident: &syn::Ident, fields: &syn::Fields) -> TokenStream {
    let rejection = match fields {
        syn::Fields::Named(_) | syn::Fields::Unnamed(_) => quote! {
            <::axum::extract::Path<Self> as ::axum::extract::FromRequest<B>>::Rejection
        },
        syn::Fields::Unit => quote! { ::std::convert::Infallible },
    };

    let from_request_body = match fields {
        syn::Fields::Named(_) | syn::Fields::Unnamed(_) => quote! {
            ::axum::extract::FromRequest::from_request(req)
                .await
                .map(|::axum::extract::Path(inner)| inner)
        },
        syn::Fields::Unit => quote! {
            Ok(Self)
        },
    };

    quote! {
        #[::axum::async_trait]
        #[automatically_derived]
        impl<B> ::axum::extract::FromRequest<B> for #ident
        where
            B: Send,
        {
            type Rejection = #rejection;

            async fn from_request(
                req: &mut ::axum::extract::RequestParts<B>,
            ) -> ::std::result::Result<Self, Self::Rejection> {
                #from_request_body
            }
        }
    }
}
