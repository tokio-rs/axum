use crate::util::PercentDecodedStr;
use http::Extensions;
use matchit::Params;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct UrlParams {
    params: Vec<(Arc<str>, PercentDecodedStr)>,
    inner_start: usize,
    /// Indices into `params` of captures that weren't valid UTF-8 after percent decoding.
    ///
    /// Those captures are still stored (lossily decoded) so that indices line up with the
    /// number of captures, but must never be handed out to extractors.
    invalid_utf8: Vec<usize>,
}

impl UrlParams {
    /// All captures, or the key of the first one that isn't valid UTF-8.
    pub(crate) fn all(&self) -> Result<&[(Arc<str>, PercentDecodedStr)], &Arc<str>> {
        self.checked(0)
    }

    /// Captures of the innermost router, or the key of the first one of those that isn't valid
    /// UTF-8.
    ///
    /// Invalid captures of enclosing routers are ignored.
    pub(crate) fn inner(&self) -> Result<&[(Arc<str>, PercentDecodedStr)], &Arc<str>> {
        self.checked(self.inner_start)
    }

    fn checked(&self, start: usize) -> Result<&[(Arc<str>, PercentDecodedStr)], &Arc<str>> {
        if let Some(&idx) = self.invalid_utf8.iter().find(|&&idx| idx >= start) {
            return Err(&self.params[idx].0);
        }

        Ok(self.params.get(start..).expect(
            "Mismatch between url params and count of captures. This is bug in axum. Please file an issue.",
        ))
    }
}

pub(super) fn insert_url_params(extensions: &mut Extensions, params: &Params<'_, '_>) {
    let url_params = extensions.get_or_insert_with(|| UrlParams {
        params: Vec::new(),
        inner_start: 0,
        invalid_utf8: Vec::new(),
    });

    let params = params
        .iter()
        .filter(|(key, _)| !key.starts_with(super::NEST_TAIL_PARAM))
        .filter(|(key, _)| !key.starts_with(super::FALLBACK_PARAM));

    for (k, v) in params {
        let decoded = PercentDecodedStr::new(v).unwrap_or_else(|| {
            url_params.invalid_utf8.push(url_params.params.len());
            PercentDecodedStr::new_lossy(v)
        });
        url_params.params.push((Arc::from(k), decoded));
    }
}

pub(super) fn advance_inner_start(extensions: &mut Extensions, count: usize) {
    if let Some(UrlParams { inner_start, .. }) = extensions.get_mut() {
        *inner_start += count;
    }
}
