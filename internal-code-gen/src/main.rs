//! This is an internal crate that generates a bunch of boilerplate code.
//!
//! Mostly implementing traits for various sizes of tuples.
//!
//! We cannot do this with a declarative macro because those cannot generate trait bounds, i.e.
//! this isn't allowed:
//!
//! ```ignore
//! macro_rules! make_bound {
//!     () => {};
//! }
//!
//! fn foo()
//! where
//!     make_bound!()
//! {}
//! ```
//!
//! So instead we generate all the code and save it to a file.

use quote::{format_ident, quote};

fn main() {
    code_gen_handler();
    code_gen_from_request_tuples();
    format();
}

/// The max tuples we implement things for
const SIZE: usize = 16;

fn code_gen_handler() {
    let mut acc = String::new();

    for n in 1..=SIZE {
        let tys = (1..=n).map(|n| format_ident!("T{}", n)).collect::<Vec<_>>();

        let mut bounds = quote! {};
        let mut mut_body = quote! {};
        let mut once_body = quote! {};

        let mut tys_iter = tys.clone().into_iter().peekable();
        while let Some(ty) = tys_iter.next() {
            if tys_iter.peek().is_some() {
                // not the last one
                bounds.extend(quote! {
                    #ty: FromRequest<Mut, B> + Send,
                });

                mut_body.extend(quote! {
                    let #ty = match #ty::from_request(&mut req).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_response(),
                    };
                });
            } else {
                // the last one
                bounds.extend(quote! {
                    #ty: FromRequest<Once, B> + Send,
                });

                once_body.extend(quote! {
                    let #ty = match #ty::from_request(&mut req).await {
                        Ok(value) => value,
                        Err(rejection) => return rejection.into_response(),
                    };
                });
            }
        }

        let code = quote! {
            impl<F, Fut, Res, B, #(#tys,)*> Handler<(#(#tys,)*), B> for F
            where
                F: FnOnce(#(#tys,)*) -> Fut + Clone + Send + 'static,
                Fut: Future<Output = Res> + Send,
                Res: IntoResponse,
                B: Send + 'static,
                #bounds
            {
                type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

                #[allow(non_snake_case, unused_mut)]
                fn call(self, req: Request<B>) -> Self::Future {
                    Box::pin(async move {
                        let mut req = RequestParts::<Mut, B>::new(req);
                        #mut_body

                        let mut req = RequestParts::<Once, B>::new(req.into_request());
                        #once_body

                        let res = self(#(#tys,)*).await;
                        res.into_response()
                    })
                }
            }
        };

        acc.push_str(&code.to_string());
    }

    let acc = format!(
        "// this file is machine generated. Don't edit it!\n\nuse super::*;\n\n{}",
        acc
    );

    std::fs::write("axum/src/handler/generated.rs", acc).unwrap();
}

fn code_gen_from_request_tuples() {
    let mut acc = String::new();

    for n in 1..=SIZE {
        let tys = (1..=n).map(|n| format_ident!("T{}", n)).collect::<Vec<_>>();

        let mut bounds = quote! {};
        let mut mut_body = quote! {};
        let mut once_body = quote! {};

        let mut tys_iter = tys.clone().into_iter().peekable();
        while let Some(ty) = tys_iter.next() {
            if tys_iter.peek().is_some() {
                // not the last one
                bounds.extend(quote! {
                    #ty: FromRequest<Mut, B> + Send,
                });

                mut_body.extend(quote! {
                    let #ty = #ty::from_request(&mut req)
                        .await
                        .map_err(|err| err.into_response())?;
                });
            } else {
                // the last one
                bounds.extend(quote! {
                    #ty: FromRequest<Once, B> + Send,
                });

                once_body.extend(quote! {
                    let #ty = #ty::from_request(&mut req)
                        .await
                        .map_err(|err| err.into_response())?;

                });
            }
        }

        let code = quote! {
            #[async_trait]
            impl<B, #(#tys,)*> FromRequest<Once, B> for (#(#tys,)*)
            where
                B: Send,
                #bounds
            {
                type Rejection = Response;

                #[allow(non_snake_case, unused_mut)]
                async fn from_request(req: &mut RequestParts<Once, B>) -> Result<Self, Self::Rejection> {
                    let mut req = req.to_mut();
                    #mut_body

                    let mut req = RequestParts::<Once, B>::new(req.into_request());
                    #once_body

                    Ok((#(#tys,)*))
                }
            }
        };

        acc.push_str(&code.to_string());
    }

    let acc = format!(
        "// this file is machine generated. Don't edit it!\n\nuse super::*;\nuse crate::response::Response;\n\n{}",
        acc
    );

    std::fs::write("axum-core/src/extract/tuple.rs", acc).unwrap();
}

fn format() {
    let status = std::process::Command::new("cargo")
        .arg("fmt")
        .status()
        .unwrap();
    assert!(status.success());
}
