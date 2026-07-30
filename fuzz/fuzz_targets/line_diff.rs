#![no_main]

use libfuzzer_sys::fuzz_target;
use revier_analysis::overlay::line_diff::diff_lines;

const MAX_INPUT_BYTES: usize = 16 * 1024;

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_INPUT_BYTES)];
    let split = data.len() / 2;
    let old_text = String::from_utf8_lossy(&data[..split]);
    let new_text = String::from_utf8_lossy(&data[split..]);
    let _ = diff_lines(&old_text, &new_text);
});
