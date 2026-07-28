use proptest::prelude::*;
use revier_analysis::contracts::{ResolvedTextEncoding, TextEncoding};
use revier_analysis::text_encoding::decode_text_bytes;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn explicit_utf8_decoding_round_trips_generated_text(text in generated_text()) {
        let decoded = decode_text_bytes(text.as_bytes(), TextEncoding::Utf8)
            .expect("生成的 UTF-8 文本应可显式解码");

        prop_assert_eq!(decoded.text, text);
        prop_assert_eq!(decoded.encoding, ResolvedTextEncoding::Utf8);
    }

    #[test]
    fn auto_decoding_accepts_generated_utf8_without_nul(text in generated_text()) {
        let decoded = decode_text_bytes(text.as_bytes(), TextEncoding::Auto)
            .expect("生成的无 NUL UTF-8 文本应可自动识别");

        prop_assert_eq!(decoded.text, text);
        prop_assert_eq!(decoded.encoding, ResolvedTextEncoding::Utf8);
    }
}

fn generated_text() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            Just('a'),
            Just('b'),
            Just('Z'),
            Just('0'),
            Just(' '),
            Just('\n'),
            Just('\t'),
            Just('你'),
            Just('界'),
        ],
        0..200,
    )
    .prop_map(|chars| chars.into_iter().collect())
}
