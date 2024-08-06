use http::Extensions;
use matchit::Params;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) enum UrlParams {
    Params(Vec<(Arc<str>, Arc<str>)>),
}

pub(super) fn insert_url_params(extensions: &mut Extensions, params: Params) {
    let current_params = extensions.get_mut();

    let params = params
        .iter()
        .filter(|(key, _)| !key.starts_with(super::NEST_TAIL_PARAM))
        .filter(|(key, _)| !key.starts_with(super::FALLBACK_PARAM))
        .map(|(k, v)| {
            (
                Arc::from(k),
                Arc::from(v.replace("%2f", "/").replace("%2F", "/")),
            )
        })
        .collect::<Vec<_>>();

    match (current_params, params) {
        (Some(UrlParams::Params(current)), params) => {
            current.extend(params);
        }
        (None, params) => {
            extensions.insert(UrlParams::Params(params));
        }
    }
}
