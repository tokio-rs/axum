use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    Token,
};

const GENERICS_ERROR: &str = "`#[derive(FromRequest)] doesn't support generics";

pub(crate) fn expand(item: syn::ItemStruct) -> syn::Result<TokenStream> {
    let syn::ItemStruct {
        attrs,
        ident,
        generics,
        fields,
        semi_token: _,
        vis,
        struct_token: _,
    } = item;

    if !generics.params.is_empty() {
        return Err(syn::Error::new_spanned(generics, GENERICS_ERROR));
    }

    if let Some(where_clause) = generics.where_clause {
        return Err(syn::Error::new_spanned(where_clause, GENERICS_ERROR));
    }

    let FromRequestAttrs { via } = parse_attrs(&attrs)?;

    if let Some((_, path)) = via {
        impl_by_extracting_all_at_once(ident, fields, path)
    } else {
        impl_by_extracting_each_field(ident, fields, vis)
    }
}

fn impl_by_extracting_each_field(
    ident: syn::Ident,
    fields: syn::Fields,
    vis: syn::Visibility,
) -> syn::Result<TokenStream> {
    let extract_fields = extract_fields(&fields)?;

    let (rejection_ident, rejection) = if let syn::Fields::Unit = &fields {
        (syn::parse_quote!(::std::convert::Infallible), quote! {})
    } else {
        let rejection_ident = rejection_ident(&ident);
        let rejection = extract_each_field_rejection(&ident, &fields, &vis)?;
        (rejection_ident, rejection)
    };

    Ok(quote! {
        #[::axum::async_trait]
        impl<B> ::axum::extract::FromRequest<B> for #ident
        where
            B: ::axum::body::HttpBody + ::std::marker::Send + 'static,
            B::Data: ::std::marker::Send,
            B::Error: ::std::convert::Into<::axum::BoxError>,
        {
            type Rejection = #rejection_ident;

            async fn from_request(
                req: &mut ::axum::extract::RequestParts<B>,
            ) -> ::std::result::Result<Self, Self::Rejection> {
                ::std::result::Result::Ok(Self {
                    #(#extract_fields)*
                })
            }
        }

        #rejection
    })
}

fn rejection_ident(ident: &syn::Ident) -> syn::Type {
    let ident = quote::format_ident!("{}Rejection", ident);
    syn::parse_quote!(#ident)
}

fn extract_fields(fields: &syn::Fields) -> syn::Result<Vec<TokenStream>> {
    fields
        .iter()
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

            let into_inner = if let Some((_, path)) = via {
                let span = path.span();
                quote_spanned! {span=>
                    |#path(inner)| inner
                }
            } else {
                quote_spanned! {ty_span=>
                    ::std::convert::identity
                }
            };

            let rejection_variant_name = rejection_variant_name(field)?;

            Ok(quote_spanned! {ty_span=>
                #member: {
                    ::axum::extract::FromRequest::from_request(req)
                        .await
                        .map(#into_inner)
                        .map_err(Self::Rejection::#rejection_variant_name)?
                },
            })
        })
        .collect()
}

fn extract_each_field_rejection(
    ident: &syn::Ident,
    fields: &syn::Fields,
    vis: &syn::Visibility,
) -> syn::Result<TokenStream> {
    let rejection_ident = rejection_ident(ident);

    let variants = fields
        .iter()
        .map(|field| {
            let FromRequestAttrs { via } = parse_attrs(&field.attrs)?;

            let ty = &field.ty;
            let ty_span = ty.span();

            let variant_name = rejection_variant_name(field)?;

            if let Some((_, path)) = via {
                Ok(quote_spanned! {ty_span=>
                    #variant_name(<#path<#ty> as ::axum::extract::FromRequest<::axum::body::Body>>::Rejection),
                })
            } else {
                Ok(quote_spanned! {ty_span=>
                    #variant_name(<#ty as ::axum::extract::FromRequest<::axum::body::Body>>::Rejection),
                })
            }
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let impl_into_response = {
        let arms = fields
            .iter()
            .map(|field| {
                let variant_name = rejection_variant_name(field)?;
                Ok(quote! {
                    Self::#variant_name(inner) => inner.into_response(),
                })
            })
            .collect::<syn::Result<Vec<_>>>()?;

        quote! {
            impl ::axum::response::IntoResponse for #rejection_ident {
                fn into_response(self) -> ::axum::response::Response {
                    match self {
                        #(#arms)*
                    }
                }
            }
        }
    };

    let impl_display = {
        let arms = fields
            .iter()
            .map(|field| {
                let variant_name = rejection_variant_name(field)?;
                Ok(quote! {
                    Self::#variant_name(inner) => inner.fmt(f),
                })
            })
            .collect::<syn::Result<Vec<_>>>()?;

        quote! {
            impl ::std::fmt::Display for #rejection_ident {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    match self {
                        #(#arms)*
                    }
                }
            }
        }
    };

    let impl_error = {
        let arms = fields
            .iter()
            .map(|field| {
                let variant_name = rejection_variant_name(field)?;
                Ok(quote! {
                    Self::#variant_name(inner) => Some(inner),
                })
            })
            .collect::<syn::Result<Vec<_>>>()?;

        quote! {
            impl ::std::error::Error for #rejection_ident {
                fn source(&self) -> ::std::option::Option<&(dyn ::std::error::Error + 'static)> {
                    match self {
                        #(#arms)*
                    }
                }
            }
        }
    };

    Ok(quote! {
        #[derive(Debug)]
        #vis enum #rejection_ident {
            #(#variants)*
        }

        #impl_into_response
        #impl_display
        #impl_error
    })
}

fn rejection_variant_name(field: &syn::Field) -> syn::Result<&syn::Ident> {
    if let syn::Type::Path(type_path) = &field.ty {
        Ok(&type_path
            .path
            .segments
            .last()
            .ok_or_else(|| syn::Error::new_spanned(&field.ty, "Empty type path"))?
            .ident)
    } else {
        Err(syn::Error::new_spanned(&field.ty, "Expected type path"))
    }
}

fn impl_by_extracting_all_at_once(
    ident: syn::Ident,
    fields: syn::Fields,
    path: syn::Path,
) -> syn::Result<TokenStream> {
    let fields = match fields {
        syn::Fields::Named(fields) => fields.named.into_iter(),
        syn::Fields::Unnamed(fields) => fields.unnamed.into_iter(),
        syn::Fields::Unit => Punctuated::<_, Token![,]>::new().into_iter(),
    };

    for field in fields {
        let FromRequestAttrs { via } = parse_attrs(&field.attrs)?;
        if let Some((via, _)) = via {
            return Err(syn::Error::new_spanned(
                via,
                "`#[from_request(via(...))]` on a field cannot be used \
                together with `#[from_request(...)]` on the container",
            ));
        }
    }

    let path_span = path.span();

    Ok(quote_spanned! {path_span=>
        #[::axum::async_trait]
        impl<B> ::axum::extract::FromRequest<B> for #ident
        where
            B: ::axum::body::HttpBody + ::std::marker::Send + 'static,
            B::Data: ::std::marker::Send,
            B::Error: ::std::convert::Into<::axum::BoxError>,
        {
            type Rejection = <#path<Self> as ::axum::extract::FromRequest<B>>::Rejection;

            async fn from_request(
                req: &mut ::axum::extract::RequestParts<B>,
            ) -> ::std::result::Result<Self, Self::Rejection> {
                ::axum::extract::FromRequest::<B>::from_request(req)
                    .await
                    .map(|#path(inner)| inner)
            }
        }
    })
}

#[derive(Debug, Default)]
struct FromRequestAttrs {
    via: Option<(kw::via, syn::Path)>,
}

mod kw {
    syn::custom_keyword!(via);
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

    let attrs = attrs
        .iter()
        .filter_map(|attr| attr.path.get_ident().map(|ident| (ident, attr)))
        .filter_map(|(ident, attr)| {
            if ident == "from_request" {
                Some(
                    attr.parse_args_with(Punctuated::parse_terminated)
                        .map(Attr::FromRequest),
                )
            } else {
                None
            }
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let mut out = FromRequestAttrs::default();
    for attr in attrs {
        match attr {
            Attr::FromRequest(from_request_attrs) => {
                for from_request_attr in from_request_attrs {
                    match from_request_attr {
                        FromRequestAttr::Via { via, path } => {
                            if out.via.is_some() {
                                return Err(syn::Error::new_spanned(
                                    via,
                                    "`via` specified more than once",
                                ));
                            } else {
                                out.via = Some((via, path));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(out)
}
