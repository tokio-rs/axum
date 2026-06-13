#![no_main]
use libfuzzer_sys::fuzz_target;
use http::{HeaderMap, HeaderName, HeaderValue};

fuzz_target!(|data: &[u8]| {
    // Header parsing — pre-auth boundary
    if data.len() < 2 { return; }
    let (name_len, rest) = (data[0] as usize % 64, &data[1..]);
    if rest.len() < name_len { return; }
    let (name_bytes, value_bytes) = rest.split_at(name_len);
    
    let _ = HeaderName::from_bytes(name_bytes).map(|name| {
        let _ = HeaderValue::from_bytes(value_bytes).map(|val| {
            let mut headers = HeaderMap::new();
            headers.insert(name, val);
        });
    });
});
