use crate::contracts::{ResolvedTextEncoding, TextEncoding};
use crate::error::AppError;

#[derive(Debug, PartialEq, Eq)]
pub struct DecodedText {
    pub text: String,
    pub encoding: ResolvedTextEncoding,
}

pub fn parse_text_encoding(value: &str) -> Result<TextEncoding, AppError> {
    match value {
        "auto" => Ok(TextEncoding::Auto),
        "utf-8" => Ok(TextEncoding::Utf8),
        "gb18030" => Ok(TextEncoding::Gb18030),
        "utf-16le" => Ok(TextEncoding::Utf16Le),
        "utf-16be" => Ok(TextEncoding::Utf16Be),
        _ => Err(AppError::InvalidArgument(format!(
            "不支持的文本编码 {value}；支持值：auto、utf-8、gb18030、utf-16le、utf-16be"
        ))),
    }
}

pub fn decode_text_bytes(bytes: &[u8], requested: TextEncoding) -> Result<DecodedText, AppError> {
    match requested {
        TextEncoding::Auto => decode_auto(bytes),
        TextEncoding::Utf8 => decode_utf8(bytes),
        TextEncoding::Gb18030 => decode_gb18030(bytes),
        TextEncoding::Utf16Le => decode_utf16(bytes, true),
        TextEncoding::Utf16Be => decode_utf16(bytes, false),
    }
}

pub fn resolved_as_requested(encoding: ResolvedTextEncoding) -> TextEncoding {
    match encoding {
        ResolvedTextEncoding::Utf8 => TextEncoding::Utf8,
        ResolvedTextEncoding::Gb18030 => TextEncoding::Gb18030,
        ResolvedTextEncoding::Utf16Le => TextEncoding::Utf16Le,
        ResolvedTextEncoding::Utf16Be => TextEncoding::Utf16Be,
    }
}

pub fn resolved_as_str(encoding: ResolvedTextEncoding) -> &'static str {
    match encoding {
        ResolvedTextEncoding::Utf8 => "utf-8",
        ResolvedTextEncoding::Gb18030 => "gb18030",
        ResolvedTextEncoding::Utf16Le => "utf-16le",
        ResolvedTextEncoding::Utf16Be => "utf-16be",
    }
}

fn decode_auto(bytes: &[u8]) -> Result<DecodedText, AppError> {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return decode_utf8(&bytes[3..]);
    }
    if bytes.starts_with(&[0xff, 0xfe]) {
        return decode_utf16(&bytes[2..], true);
    }
    if bytes.starts_with(&[0xfe, 0xff]) {
        return decode_utf16(&bytes[2..], false);
    }
    if contains_nul(bytes) {
        return Err(binary_error());
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => decoded_text(text.to_string(), ResolvedTextEncoding::Utf8),
        Err(error) => Err(AppError::FileNotAnalyzable(format!(
            "文本没有 BOM 且不是合法 UTF-8，无法确定编码：{error}"
        ))),
    }
}

fn decode_utf8(bytes: &[u8]) -> Result<DecodedText, AppError> {
    reject_utf16_bom(bytes, "UTF-8")?;
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    if contains_nul(bytes) {
        return Err(binary_error());
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|error| AppError::FileNotAnalyzable(format!("文本不是合法 UTF-8：{error}")))?;
    decoded_text(text.to_string(), ResolvedTextEncoding::Utf8)
}

fn decode_gb18030(bytes: &[u8]) -> Result<DecodedText, AppError> {
    reject_utf16_bom(bytes, "GB18030")?;
    let text = encoding_rs::GB18030
        .decode_without_bom_handling_and_without_replacement(bytes)
        .ok_or_else(|| AppError::FileNotAnalyzable("文本不是合法 GB18030".to_string()))?;
    decoded_text(text.into_owned(), ResolvedTextEncoding::Gb18030)
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<DecodedText, AppError> {
    let conflicting_bom = if little_endian {
        [0xfe, 0xff]
    } else {
        [0xff, 0xfe]
    };
    if bytes.starts_with(&conflicting_bom) {
        return Err(AppError::FileNotAnalyzable(format!(
            "请求的 UTF-16{} 编码与文件 BOM 冲突",
            if little_endian { "LE" } else { "BE" }
        )));
    }
    let bytes = if little_endian {
        bytes.strip_prefix(&[0xff, 0xfe]).unwrap_or(bytes)
    } else {
        bytes.strip_prefix(&[0xfe, 0xff]).unwrap_or(bytes)
    };
    if bytes.len() % 2 != 0 {
        return Err(AppError::FileNotAnalyzable(
            "UTF-16 文本包含奇数字节".to_string(),
        ));
    }
    let units = bytes.chunks_exact(2).map(|pair| {
        if little_endian {
            u16::from_le_bytes([pair[0], pair[1]])
        } else {
            u16::from_be_bytes([pair[0], pair[1]])
        }
    });
    let text = String::from_utf16(&units.collect::<Vec<_>>())
        .map_err(|error| AppError::FileNotAnalyzable(format!("文本不是合法 UTF-16：{error}")))?;
    decoded_text(
        text,
        if little_endian {
            ResolvedTextEncoding::Utf16Le
        } else {
            ResolvedTextEncoding::Utf16Be
        },
    )
}

fn contains_nul(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|byte| *byte == 0)
}

fn binary_error() -> AppError {
    AppError::FileNotAnalyzable("文件包含明显二进制 NUL 字节".to_string())
}

fn decoded_text(text: String, encoding: ResolvedTextEncoding) -> Result<DecodedText, AppError> {
    reject_obvious_binary_text(&text)?;
    Ok(DecodedText { text, encoding })
}

fn reject_obvious_binary_text(text: &str) -> Result<(), AppError> {
    let mut character_count = 0_usize;
    let mut suspicious_control_count = 0_usize;
    for character in text.chars() {
        character_count += 1;
        if character == '\0' {
            return Err(binary_error());
        }
        if character.is_control() && !matches!(character, '\t' | '\n' | '\r') {
            suspicious_control_count += 1;
        }
    }
    if suspicious_control_count >= 3 && suspicious_control_count * 10 >= character_count * 3 {
        return Err(AppError::FileNotAnalyzable(
            "文本包含密集的明显控制字符".to_string(),
        ));
    }
    Ok(())
}

fn reject_utf16_bom(bytes: &[u8], requested: &str) -> Result<(), AppError> {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        return Err(AppError::FileNotAnalyzable(format!(
            "请求的 {requested} 编码与文件 UTF-16 BOM 冲突"
        )));
    }
    Ok(())
}
