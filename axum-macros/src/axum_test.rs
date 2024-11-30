use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse::Parse, parse_quote, visit_mut::VisitMut, ItemFn};

pub(crate) fn expand(_attr: Attrs, mut item_fn: ItemFn) -> TokenStream {
    item_fn.attrs.push(parse_quote!(#[tokio::test]));

    let nest_service_fn = replace_nest_with_nest_service(item_fn.clone());

    quote! {
        #item_fn
        #nest_service_fn
    }
}

pub(crate) struct Attrs;

impl Parse for Attrs {
    fn parse(_input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self)
    }
}

fn replace_nest_with_nest_service(mut item_fn: ItemFn) -> Option<ItemFn> {
    item_fn.sig.ident = format_ident!("{}_with_nest_service", item_fn.sig.ident);

    let mut visitor = NestToNestService::default();
    syn::visit_mut::visit_item_fn_mut(&mut visitor, &mut item_fn);

    (visitor.count > 0).then_some(item_fn)
}

#[derive(Default)]
struct NestToNestService {
    count: usize,
}

impl VisitMut for NestToNestService {
    fn visit_expr_method_call_mut(&mut self, i: &mut syn::ExprMethodCall) {
        if i.method == "nest" && i.args.len() == 2 {
            i.method = parse_quote!(nest_service);
            self.count += 1;
        }
    }
}
