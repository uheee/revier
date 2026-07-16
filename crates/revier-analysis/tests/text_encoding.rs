use revier_analysis::contracts::{ResolvedTextEncoding, TextEncoding};
use revier_analysis::error::AppError;
use revier_analysis::text_encoding::{
    decode_text_bytes, parse_text_encoding, resolved_as_requested, resolved_as_str,
};

#[test]
fn auto_去除_utf8_bom() {
    let decoded =
        decode_text_bytes(b"\xef\xbb\xbfhello", TextEncoding::Auto).expect("解码 UTF-8 BOM");
    assert_eq!(decoded.text, "hello");
    assert_eq!(decoded.encoding, ResolvedTextEncoding::Utf8);
}

#[test]
fn auto_解码无_bom_合法_utf8() {
    let decoded = decode_text_bytes("你好".as_bytes(), TextEncoding::Auto).expect("解码 UTF-8");
    assert_eq!(decoded.text, "你好");
    assert_eq!(decoded.encoding, ResolvedTextEncoding::Utf8);
}

#[test]
fn 手动解码_gb18030() {
    let bytes = [0xc4, 0xe3, 0xba, 0xc3];
    let decoded = decode_text_bytes(&bytes, TextEncoding::Gb18030).expect("解码 GB18030");
    assert_eq!(decoded.text, "你好");
    assert_eq!(decoded.encoding, ResolvedTextEncoding::Gb18030);
}

#[test]
fn auto_按_bom_解码_utf16_le_和_be() {
    let le = decode_text_bytes(&[0xff, 0xfe, 0x60, 0x4f, 0x7d, 0x59], TextEncoding::Auto)
        .expect("解码 UTF-16 LE BOM");
    assert_eq!(le.text, "你好");
    assert_eq!(le.encoding, ResolvedTextEncoding::Utf16Le);

    let be = decode_text_bytes(&[0xfe, 0xff, 0x4f, 0x60, 0x59, 0x7d], TextEncoding::Auto)
        .expect("解码 UTF-16 BE BOM");
    assert_eq!(be.text, "你好");
    assert_eq!(be.encoding, ResolvedTextEncoding::Utf16Be);
}

#[test]
fn 手动解码无_bom_utf16_le_和_be() {
    let le = decode_text_bytes(&[0x60, 0x4f, 0x7d, 0x59], TextEncoding::Utf16Le)
        .expect("解码 UTF-16 LE");
    assert_eq!(le.text, "你好");

    let be = decode_text_bytes(&[0x4f, 0x60, 0x59, 0x7d], TextEncoding::Utf16Be)
        .expect("解码 UTF-16 BE");
    assert_eq!(be.text, "你好");
}

#[test]
fn utf16_奇数字节失败() {
    let error = decode_text_bytes(&[0x60, 0x4f, 0x7d], TextEncoding::Utf16Le)
        .expect_err("奇数字节必须失败");
    assert!(matches!(error, AppError::FileNotAnalyzable(_)));
}

#[test]
fn 错误手动编码严格失败且不产生替换字符() {
    let error = decode_text_bytes(&[0xff], TextEncoding::Utf8).expect_err("无效 UTF-8 必须失败");
    assert!(matches!(error, AppError::FileNotAnalyzable(message) if !message.contains('\u{fffd}')));

    let error =
        decode_text_bytes(&[0x81], TextEncoding::Gb18030).expect_err("不完整 GB18030 序列必须失败");
    assert!(matches!(error, AppError::FileNotAnalyzable(message) if !message.contains('\u{fffd}')));
}

#[test]
fn auto_不猜测无_bom_非_utf8() {
    let error = decode_text_bytes(&[0xc4, 0xe3, 0xba, 0xc3], TextEncoding::Auto)
        .expect_err("auto 不应猜测 GB18030");
    assert!(
        matches!(error, AppError::FileNotAnalyzable(message) if message.contains("无法确定编码"))
    );
}

#[test]
fn auto_和_utf8_拒绝明显二进制_nul() {
    for encoding in [TextEncoding::Auto, TextEncoding::Utf8] {
        let error = decode_text_bytes(b"ab\0cd", encoding).expect_err("NUL 内容必须失败");
        assert!(matches!(error, AppError::FileNotAnalyzable(_)));
    }
}

#[test]
fn 手动_utf16_不被交错_nul_启发式提前拒绝() {
    let decoded =
        decode_text_bytes(&[0x41, 0, 0x42, 0], TextEncoding::Utf16Le).expect("合法 UTF-16 LE");
    assert_eq!(decoded.text, "AB");
}

#[test]
fn utf16_bom_解码后拒绝密集控制字符() {
    let error = decode_text_bytes(
        &[0xff, 0xfe, 0x01, 0x00, 0x02, 0x00, 0x03, 0x00, 0x41, 0x00],
        TextEncoding::Auto,
    )
    .expect_err("密集控制字符必须判定为二进制");
    assert!(matches!(error, AppError::FileNotAnalyzable(_)));
}

#[test]
fn 手动_utf16_解码后拒绝_unicode_nul() {
    let error = decode_text_bytes(&[0x41, 0x00, 0x00, 0x00, 0x42, 0x00], TextEncoding::Utf16Le)
        .expect_err("Unicode NUL 必须判定为二进制");
    assert!(matches!(error, AppError::FileNotAnalyzable(_)));
}

#[test]
fn 手动编码拒绝冲突_bom() {
    for (bytes, encoding) in [
        (&[0xfe, 0xff, 0x00, 0x41][..], TextEncoding::Utf16Le),
        (&[0xff, 0xfe, 0x41, 0x00][..], TextEncoding::Utf16Be),
        (&[0xff, 0xfe, 0x41, 0x00][..], TextEncoding::Utf8),
        (&[0xfe, 0xff, 0x00, 0x41][..], TextEncoding::Gb18030),
    ] {
        let error = decode_text_bytes(bytes, encoding).expect_err("冲突 BOM 必须失败");
        assert!(
            matches!(error, AppError::FileNotAnalyzable(message) if message.contains("BOM") && message.contains("冲突"))
        );
    }
}

#[test]
fn 解析编码仅接受已确认值() {
    assert_eq!(parse_text_encoding("auto").unwrap(), TextEncoding::Auto);
    assert_eq!(parse_text_encoding("utf-8").unwrap(), TextEncoding::Utf8);
    assert_eq!(
        parse_text_encoding("gb18030").unwrap(),
        TextEncoding::Gb18030
    );
    assert_eq!(
        parse_text_encoding("utf-16le").unwrap(),
        TextEncoding::Utf16Le
    );
    assert_eq!(
        parse_text_encoding("utf-16be").unwrap(),
        TextEncoding::Utf16Be
    );
    let error = parse_text_encoding("utf8").expect_err("未确认别名必须失败");
    assert!(
        matches!(error, AppError::InvalidArgument(message) if message.contains("auto、utf-8、gb18030、utf-16le、utf-16be"))
    );
}

#[test]
fn resolved_编码可映射为严格解码请求和值() {
    for (resolved, requested, value) in [
        (ResolvedTextEncoding::Utf8, TextEncoding::Utf8, "utf-8"),
        (
            ResolvedTextEncoding::Gb18030,
            TextEncoding::Gb18030,
            "gb18030",
        ),
        (
            ResolvedTextEncoding::Utf16Le,
            TextEncoding::Utf16Le,
            "utf-16le",
        ),
        (
            ResolvedTextEncoding::Utf16Be,
            TextEncoding::Utf16Be,
            "utf-16be",
        ),
    ] {
        assert_eq!(resolved_as_requested(resolved), requested);
        assert_eq!(resolved_as_str(resolved), value);
    }
}
