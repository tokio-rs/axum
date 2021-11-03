use crate::{
    handler_method_routing::{MethodRouter, WithOperation},
    ToOperation,
};
use axum::routing::MethodNotAllowed;
use okapi::openapi3::{Components, Operation, PathItem};

pub trait ToPathItem {
    fn to_path_item(&self, components: &mut Components) -> PathItem;
}

impl<Delete, Get, Head, Options, Patch, Post, Put, Trace, B> ToPathItem
    for MethodRouter<Delete, Get, Head, Options, Patch, Post, Put, Trace, B>
{
    fn to_path_item(&self, components_out: &mut Components) -> PathItem {
        let mut components = Some(std::mem::take(components_out));

        let delete = self.delete.as_ref().map(|op| op.to_inner());
        let get = self.get.as_ref().map(|op| op.to_inner());
        let head = self.head.as_ref().map(|op| op.to_inner());
        let options = self.options.as_ref().map(|op| op.to_inner());
        let patch = self.patch.as_ref().map(|op| op.to_inner());
        let post = self.post.as_ref().map(|op| op.to_inner());
        let put = self.put.as_ref().map(|op| op.to_inner());
        let trace = self.trace.as_ref().map(|op| op.to_inner());

        fn get_operation_and_merge_components(
            op_and_comp: Option<(Operation, Components)>,
            comp_out: &mut Option<Components>,
        ) -> Option<Operation> {
            op_and_comp.map(|(op, comp)| {
                okapi::merge::merge_components(comp_out, &Some(comp))
                    .expect("failed to merge components");
                op
            })
        }

        let item = PathItem {
            get: get_operation_and_merge_components(get, &mut components),
            delete: get_operation_and_merge_components(delete, &mut components),
            head: get_operation_and_merge_components(head, &mut components),
            options: get_operation_and_merge_components(options, &mut components),
            patch: get_operation_and_merge_components(patch, &mut components),
            put: get_operation_and_merge_components(put, &mut components),
            post: get_operation_and_merge_components(post, &mut components),
            trace: get_operation_and_merge_components(trace, &mut components),
            ..Default::default()
        };

        *components_out = components.expect("components None after merge");

        item
    }
}
