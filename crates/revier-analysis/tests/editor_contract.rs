use revier_analysis::contracts::{
    EditorFontSettings, EditorSettings, EditorSettingsSnapshot, EditorSyntaxColors,
    EditorThemeColors, EditorThemeMode, EditorThemes, LargeFileSettings, ResolvedTextEncoding,
    TextEncoding,
};

fn theme_colors() -> EditorThemeColors {
    EditorThemeColors {
        workspace_background: "#111111".into(),
        panel_background: "#222222".into(),
        editor_background: "#333333".into(),
        border: "#444444".into(),
        foreground: "#eeeeee".into(),
        muted: "#999999".into(),
        accent: "#0088ff".into(),
        selection: "#0055aa".into(),
        diff_removed: "#550000".into(),
        diff_removed_strong: "#770000".into(),
        diff_removed_word: "#990000".into(),
        diff_added: "#005500".into(),
        diff_added_strong: "#007700".into(),
        diff_added_word: "#009900".into(),
        syntax: EditorSyntaxColors {
            comment: "#777777".into(),
            keyword: "#ff00ff".into(),
            string: "#00ff00".into(),
            number: "#00ffff".into(),
            r#type: "#ffff00".into(),
            function: "#0000ff".into(),
            variable: "#ffffff".into(),
        },
    }
}

#[test]
fn text_encoding_使用精确_json_值() {
    assert_eq!(
        serde_json::to_string(&TextEncoding::Auto).unwrap(),
        "\"auto\""
    );
    assert_eq!(
        serde_json::to_string(&TextEncoding::Utf8).unwrap(),
        "\"utf-8\""
    );
    assert_eq!(
        serde_json::to_string(&TextEncoding::Gb18030).unwrap(),
        "\"gb18030\""
    );
    assert_eq!(
        serde_json::to_string(&TextEncoding::Utf16Le).unwrap(),
        "\"utf-16le\""
    );
    assert_eq!(
        serde_json::to_string(&TextEncoding::Utf16Be).unwrap(),
        "\"utf-16be\""
    );
    assert_eq!(
        serde_json::to_string(&ResolvedTextEncoding::Utf8).unwrap(),
        "\"utf-8\""
    );
    assert_eq!(
        serde_json::to_string(&ResolvedTextEncoding::Gb18030).unwrap(),
        "\"gb18030\""
    );
    assert_eq!(
        serde_json::to_string(&ResolvedTextEncoding::Utf16Le).unwrap(),
        "\"utf-16le\""
    );
    assert_eq!(
        serde_json::to_string(&ResolvedTextEncoding::Utf16Be).unwrap(),
        "\"utf-16be\""
    );
}

#[test]
fn editor_settings_snapshot_序列化为驼峰字段且省略空警告() {
    let snapshot = EditorSettingsSnapshot {
        settings: EditorSettings {
            version: 1,
            theme: EditorThemeMode::System,
            default_encoding: TextEncoding::Auto,
            editor: EditorFontSettings {
                font_families: vec!["JetBrains Mono".into()],
                font_size: 14,
                line_height: 22,
                minimap: true,
            },
            large_file: LargeFileSettings {
                max_bytes: 10_000_000,
                max_lines: 100_000,
            },
            themes: EditorThemes {
                light: theme_colors(),
                dark: theme_colors(),
            },
        },
        config_path: "C:/Users/test/.revier/config.toml".into(),
        warning: None,
    };

    let json = serde_json::to_value(snapshot).unwrap();

    assert_eq!(json["configPath"], "C:/Users/test/.revier/config.toml");
    assert_eq!(json["settings"]["defaultEncoding"], "auto");
    assert_eq!(
        json["settings"]["editor"]["fontFamilies"][0],
        "JetBrains Mono"
    );
    assert_eq!(json["settings"]["largeFile"]["maxBytes"], 10_000_000);
    assert_eq!(
        json["settings"]["themes"]["light"]["workspaceBackground"],
        "#111111"
    );
    assert_eq!(
        json["settings"]["themes"]["dark"]["diffRemovedStrong"],
        "#770000"
    );
    assert!(json.get("warning").is_none());
}
