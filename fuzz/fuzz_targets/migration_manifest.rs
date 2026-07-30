#![no_main]

use libfuzzer_sys::fuzz_target;
use revier_analysis::index::migrations::validate_migration_manifest_for_fuzz;

const MAX_ENTRIES: usize = 16;
const MAX_NAME_BYTES: usize = 64;
const MAX_SQL_BYTES: usize = 256;

fuzz_target!(|data: &[u8]| {
    let mut offset = 0;
    let mut entries = Vec::new();
    while offset < data.len() && entries.len() < MAX_ENTRIES {
        let name_len = usize::from(data[offset] % (MAX_NAME_BYTES as u8 + 1));
        offset += 1;
        if offset >= data.len() {
            break;
        }
        let available_name = name_len.min(data.len() - offset);
        let name = String::from_utf8_lossy(&data[offset..offset + available_name]).to_string();
        offset += available_name;
        if offset >= data.len() {
            break;
        }
        let sql_len = usize::from(data[offset]).min(MAX_SQL_BYTES);
        offset += 1;
        let available_sql = sql_len.min(data.len() - offset);
        let sql = data[offset..offset + available_sql].to_vec();
        offset += available_sql;
        entries.push((name, sql));
    }
    let _ = validate_migration_manifest_for_fuzz(&entries);
});
