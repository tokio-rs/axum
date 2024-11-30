#![allow(clippy::disallowed_names)]

use crate::{extract::Request, response::Response, serve};

mod test_client;
pub(crate) use self::test_client::*;

pub(crate) mod tracing_helpers;

pub(crate) mod counting_cloneable_state;

pub(crate) fn assert_send<T: Send>() {}
pub(crate) fn assert_sync<T: Sync>() {}

#[allow(dead_code)]
pub(crate) struct NotSendSync(*const ());
