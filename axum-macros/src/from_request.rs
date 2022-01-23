use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    Token,
};

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
        syn::Fields::Named(fields) => extract_fields(fields.named.iter())?,
        syn::Fields::Unnamed(fields) => extract_fields(fields.unnamed.iter())?,
        syn::Fields::Unit => Default::default(),
    };

    if !generics.params.is_empty() {
        return Err(syn::Error::new_spanned(generics, GENERICS_ERROR));
    }

    if let Some(where_clause) = generics.where_clause {
        return Err(syn::Error::new_spanned(where_clause, GENERICS_ERROR));
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

const GENERICS_ERROR: &str = "`#[derive(FromRequest)] doesn't support generics";

fn extract_fields<'a, I>(fields: I) -> syn::Result<Vec<TokenStream>>
where
    I: Iterator<Item = &'a syn::Field>,
{
    fields
        .enumerate()
        .map(|(index, field)| {
            let FromRequestAttrs { via } = parse_attrs(&field.attrs)?;

            let member = if let Some(ident) = &field.ident {
                quote! { #ident }
            } else {
                let member = syn::Member::Unnamed(syn::Index {
                    index: index as u32,
                    span: field.span(),
                });
                quote! { #member }
            };

            let ty_span = field.ty.span();

            let from_request_ty = if let Some(via) = &via {
                let span = via.span();
                quote_spanned! {span=>
                    <#via<_> as ::axum::extract::FromRequest<B>>
                }
            } else {
                quote_spanned! {ty_span=>
                    ::axum::extract::FromRequest
                }
            };

            let into_inner = if let Some(via) = via {
                let span = via.span();
                quote_spanned! {span=>
                    |#via(inner)| inner
                }
            } else {
                quote_spanned! {ty_span=>
                    ::std::convert::identity
                }
            };

            Ok(quote_spanned! {ty_span=>
                #member: {
                    #from_request_ty::from_request(req)
                        .await
                        .map(#into_inner)
                        .map_err(::axum::response::IntoResponse::into_response)?
                },
            })
        })
        .collect()
}

#[derive(Debug, Default)]
struct FromRequestAttrs {
    via: Option<syn::Path>,
}

fn parse_attrs(attrs: &[syn::Attribute]) -> syn::Result<FromRequestAttrs> {
    #[derive(Debug)]
    enum Attr {
        FromRequest(Punctuated<FromRequestAttr, Token![,]>),
    }

    #[derive(Debug)]
    enum FromRequestAttr {
        Via { via: kw::via, path: syn::Path },
    }

    impl Parse for FromRequestAttr {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let lh = input.lookahead1();
            if lh.peek(kw::via) {
                let via = input.parse::<kw::via>()?;
                let content;
                syn::parenthesized!(content in input);
                content.parse().map(|path| Self::Via { via, path })
            } else {
                Err(lh.error())
            }
        }
    }

    mod kw {
        syn::custom_keyword!(via);
    }

    let attrs = attrs
        .iter()
        .filter_map(|attr| attr.path.get_ident().map(|ident| (ident, attr)))
        .map(|(ident, attr)| {
            if ident == "from_request" {
                attr.parse_args_with(Punctuated::parse_terminated)
                    .map(Attr::FromRequest)
            } else {
                Err(syn::Error::new_spanned(
                    ident,
                    format!("Unknown attribute: `{}`", ident),
                ))
            }
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let mut out = FromRequestAttrs::default();
    for attr in attrs {
        match attr {
            Attr::FromRequest(from_requst_attrs) => {
                for from_request_attr in from_requst_attrs {
                    match from_request_attr {
                        FromRequestAttr::Via { via, path } => {
                            if out.via.is_some() {
                                return Err(syn::Error::new_spanned(
                                    via,
                                    "`via` specified more than once",
                                ));
                            } else {
                                out.via = Some(path);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(out)
}
