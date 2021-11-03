use okapi::openapi3::PathItem;

pub trait ToPathItem {
    fn to_path_item(&self) -> PathItem;
}
