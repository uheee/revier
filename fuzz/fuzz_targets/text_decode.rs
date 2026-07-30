#![no_main]

use libfuzzer_sys::fuzz_target;
use revier_analysis::contracts::TextEncoding;
use revier_analysis::text_encoding::decode_text_bytes;

const MAX_INPUT_BYTES: usize = 64 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }
    let requested = match data[0] % 5 {
        0 => TextEncoding::Auto,
        1 => TextEncoding::Utf8,
        2 => TextEncoding::Gb18030,
        3 => TextEncoding::Utf16Le,
        _ => TextEncoding::Utf16Be,
    };
    let bytes = &data[1..data.len().min(MAX_INPUT_BYTES)];
    let _ = decode_text_bytes(bytes, requested);
});
