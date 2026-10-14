use crate::util::PercentDecodedStr;
use http::Extensions;
use matchit::Params;
use std::sync::Arc;

/// The captures matched for a request, in the order the routers matched them.
///
/// A capture whose percent-decoded value isn't valid UTF-8 stores `None` as
/// its value instead of discarding the other captures. That way extractors
/// which only read a subset of the captures, such as `InnerPath`, don't fail
/// because of an invalid capture outside of that subset.
#[derive(Clone)]
pub(crate) struct UrlParams {
    params: Vec<(Arc<str>, Option<PercentDecodedStr>)>,
    inner_start: usize,
}

impl UrlParams {
    /// All captures, or the key of the first capture that isn't valid UTF-8.
    pub(crate) fn all(&self) -> Result<Vec<(Arc<str>, PercentDecodedStr)>, &Arc<str>> {
        collect_valid(&self.params)
    }

    /// The captures of the innermost router, or the key of the first of those
    /// captures that isn't valid UTF-8.
    ///
    /// Captures matched by outer routers are ignored, even invalid ones.
    pub(crate) fn inner(&self) -> Result<Vec<(Arc<str>, PercentDecodedStr)>, &Arc<str>> {
        let inner_params = self.params.get(self.inner_start..).expect(
            "Mismatch between url params and count of captures. \
             This is bug in axum. Please file an issue.",
        );
        collect_valid(inner_params)
    }
}

fn collect_valid(
    params: &[(Arc<str>, Option<PercentDecodedStr>)],
) -> Result<Vec<(Arc<str>, PercentDecodedStr)>, &Arc<str>> {
    params
        .iter()
        .map(|(key, value)| match value {
            Some(value) => Ok((Arc::clone(key), value.clone())),
            None => Err(key),
        })
        .collect()
}

pub(super) fn insert_url_params(extensions: &mut Extensions, params: &Params<'_, '_>) {
    let params = params
        .iter()
        .filter(|(key, _)| !key.starts_with(super::NEST_TAIL_PARAM))
        .filter(|(key, _)| !key.starts_with(super::FALLBACK_PARAM))
        .map(|(k, v)| (Arc::from(k), PercentDecodedStr::new(v)));

    match extensions.get_mut::<UrlParams>() {
        Some(url_params) => url_params.params.extend(params),
        None => {
            extensions.insert(UrlParams {
                params: params.collect(),
                inner_start: 0,
            });
        }
    }
}

pub(super) fn advance_inner_start(extensions: &mut Extensions, count: usize) {
    if let Some(url_params) = extensions.get_mut::<UrlParams>() {
        url_params.inner_start += count;
    }
}
