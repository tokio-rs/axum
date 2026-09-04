use crate::util::PercentDecodedStr;
use http::Extensions;
use matchit::Params;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) enum UrlParams {
    Params {
        params: Vec<(Arc<str>, PercentDecodedStr)>,
        inner_start: usize,
    },
    InvalidUtf8InPathParam {
        key: Arc<str>,
    },
}

pub(super) fn insert_url_params(extensions: &mut Extensions, params: &Params<'_, '_>) {
    let current_params = extensions.get_mut();

    if let Some(UrlParams::InvalidUtf8InPathParam { .. }) = current_params {
        // nothing to do here since an error was stored earlier
        return;
    }

    let params = params
        .iter()
        .filter(|(key, _)| !key.starts_with(super::NEST_TAIL_PARAM))
        .filter(|(key, _)| !key.starts_with(super::FALLBACK_PARAM))
        .map(|(k, v)| {
            if let Some(decoded) = PercentDecodedStr::new(v) {
                Ok((Arc::from(k), decoded))
            } else {
                Err(Arc::from(k))
            }
        })
        .collect::<Result<Vec<_>, _>>();

    match (current_params, params) {
        (Some(UrlParams::InvalidUtf8InPathParam { .. }), _) => {
            unreachable!("we check for this state earlier in this method")
        }
        (_, Err(invalid_key)) => {
            extensions.insert(UrlParams::InvalidUtf8InPathParam { key: invalid_key });
        }
        (
            Some(UrlParams::Params {
                params: current, ..
            }),
            Ok(params),
        ) => {
            current.extend(params);
        }
        (None, Ok(params)) => {
            extensions.insert(UrlParams::Params {
                params,
                inner_start: 0,
            });
        }
    }
}

pub(super) fn advance_inner_start(extensions: &mut Extensions, count: usize) {
    if let Some(UrlParams::Params { inner_start, .. }) = extensions.get_mut() {
        *inner_start += count;
    }
}
