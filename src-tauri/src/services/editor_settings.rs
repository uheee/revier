use revier_analysis::contracts::{
    EditorFontSettings, EditorSettings, EditorSettingsSnapshot, EditorSyntaxColors,
    EditorThemeColors, EditorThemeMode, EditorThemes, LargeFileSettings, TextEncoding,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

pub struct EditorSettingsService {
    snapshot: EditorSettingsSnapshot,
}

impl EditorSettingsService {
    pub fn load(path: PathBuf) -> Self {
        let defaults = EditorSettingsFile::default();
        let path_text = path.to_string_lossy().into_owned();
        let result = if path.exists() {
            read_and_validate(&path)
        } else {
            write_defaults(&path, &defaults).map(|()| defaults.clone())
        };
        let (settings, warning) = match result {
            Ok(settings) => (settings.into(), None),
            Err(error) => (
                defaults.into(),
                Some(format!("编辑器配置 {} 无法使用：{error}", path.display())),
            ),
        };

        Self {
            snapshot: EditorSettingsSnapshot {
                settings,
                config_path: path_text,
                warning,
            },
        }
    }

    pub fn snapshot(&self) -> EditorSettingsSnapshot {
        self.snapshot.clone()
    }
}

fn read_and_validate(path: &PathBuf) -> Result<EditorSettingsFile, String> {
    let bytes = fs::read(path).map_err(|error| format!("读取失败：{error}"))?;
    let text =
        String::from_utf8(bytes).map_err(|error| format!("解析失败：配置不是 UTF-8：{error}"))?;
    let settings: EditorSettingsFile =
        toml::from_str(&text).map_err(|error| format!("解析失败：{error}"))?;
    settings.validate()?;
    Ok(settings)
}

fn write_defaults(path: &PathBuf, defaults: &EditorSettingsFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建配置目录失败：{error}"))?;
    }
    let text = toml::to_string_pretty(defaults).map_err(|error| format!("序列化失败：{error}"))?;
    fs::write(path, text).map_err(|error| format!("写入默认配置失败：{error}"))
}

#[derive(Clone, Serialize, Deserialize)]
struct EditorSettingsFile {
    version: u32,
    theme: EditorThemeMode,
    default_encoding: TextEncoding,
    editor: EditorFile,
    large_file: LargeFileFile,
    themes: ThemesFile,
}

#[derive(Clone, Serialize, Deserialize)]
struct EditorFile {
    font_families: Vec<String>,
    font_size: u32,
    line_height: u32,
    minimap: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct LargeFileFile {
    max_bytes: u64,
    max_lines: u64,
}

#[derive(Clone, Serialize, Deserialize)]
struct ThemesFile {
    light: ThemeColorsFile,
    dark: ThemeColorsFile,
}

#[derive(Clone, Serialize, Deserialize)]
struct ThemeColorsFile {
    workspace_background: String,
    panel_background: String,
    editor_background: String,
    border: String,
    foreground: String,
    muted: String,
    accent: String,
    selection: String,
    diff_removed: String,
    diff_removed_strong: String,
    diff_removed_word: String,
    diff_added: String,
    diff_added_strong: String,
    diff_added_word: String,
    syntax: SyntaxColorsFile,
}

#[derive(Clone, Serialize, Deserialize)]
struct SyntaxColorsFile {
    comment: String,
    keyword: String,
    string: String,
    number: String,
    r#type: String,
    function: String,
    variable: String,
}

impl EditorSettingsFile {
    fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!("version 必须为 1，当前为 {}", self.version));
        }
        if self.editor.font_size == 0 {
            return Err("editor.font_size 必须大于 0".to_string());
        }
        if self.editor.line_height == 0 {
            return Err("editor.line_height 必须大于 0".to_string());
        }
        if self.large_file.max_bytes == 0 {
            return Err("large_file.max_bytes 必须大于 0".to_string());
        }
        if self.large_file.max_lines == 0 {
            return Err("large_file.max_lines 必须大于 0".to_string());
        }
        self.themes.light.validate("themes.light")?;
        self.themes.dark.validate("themes.dark")
    }
}

impl ThemeColorsFile {
    fn validate(&self, prefix: &str) -> Result<(), String> {
        for (field, color) in [
            ("workspace_background", &self.workspace_background),
            ("panel_background", &self.panel_background),
            ("editor_background", &self.editor_background),
            ("border", &self.border),
            ("foreground", &self.foreground),
            ("muted", &self.muted),
            ("accent", &self.accent),
            ("selection", &self.selection),
            ("diff_removed", &self.diff_removed),
            ("diff_removed_strong", &self.diff_removed_strong),
            ("diff_removed_word", &self.diff_removed_word),
            ("diff_added", &self.diff_added),
            ("diff_added_strong", &self.diff_added_strong),
            ("diff_added_word", &self.diff_added_word),
        ] {
            validate_color(&format!("{prefix}.{field}"), color)?;
        }
        self.syntax.validate(&format!("{prefix}.syntax"))
    }
}

impl SyntaxColorsFile {
    fn validate(&self, prefix: &str) -> Result<(), String> {
        for (field, color) in [
            ("comment", &self.comment),
            ("keyword", &self.keyword),
            ("string", &self.string),
            ("number", &self.number),
            ("type", &self.r#type),
            ("function", &self.function),
            ("variable", &self.variable),
        ] {
            validate_color(&format!("{prefix}.{field}"), color)?;
        }
        Ok(())
    }
}

fn validate_color(field: &str, color: &str) -> Result<(), String> {
    let valid = color.len() == 7
        && color.starts_with('#')
        && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit);
    if valid {
        Ok(())
    } else {
        Err(format!(
            "{field} 必须匹配 #[0-9A-Fa-f]{{6}}，当前为 {color}"
        ))
    }
}

impl Default for EditorSettingsFile {
    fn default() -> Self {
        Self {
            version: 1,
            theme: EditorThemeMode::System,
            default_encoding: TextEncoding::Auto,
            editor: EditorFile {
                font_families: vec![
                    "JetBrainsMono Nerd Font Mono".to_string(),
                    "Microsoft YaHei".to_string(),
                    "monospace".to_string(),
                ],
                font_size: 13,
                line_height: 22,
                minimap: true,
            },
            large_file: LargeFileFile {
                max_bytes: 1_048_576,
                max_lines: 5_000,
            },
            themes: ThemesFile {
                light: ThemeColorsFile::light(),
                dark: ThemeColorsFile::dark(),
            },
        }
    }
}

impl ThemeColorsFile {
    fn light() -> Self {
        Self::new(
            [
                "#F4F6F8", "#FFFFFF", "#FCFDFE", "#DFE5EC", "#273448", "#768296", "#0F766E",
                "#DCEFEB", "#FBE7E5", "#BC3D35", "#F1B9B3", "#E2F3E8", "#26804A", "#A9DBBB",
            ],
            [
                "#768296", "#893CAD", "#0B7952", "#A05B00", "#0969DA", "#1C63A5", "#273448",
            ],
        )
    }

    fn dark() -> Self {
        Self::new(
            [
                "#111821", "#161F2A", "#101720", "#293645", "#DCE3EC", "#94A1B3", "#2DD4BF",
                "#173C3A", "#3E262C", "#E06B63", "#743A43", "#1D3A30", "#5EC58A", "#30664C",
            ],
            [
                "#94A1B3", "#D7A0F2", "#8BD7AE", "#EFB875", "#82B7FF", "#82B7FF", "#DCE3EC",
            ],
        )
    }

    fn new(colors: [&str; 14], syntax: [&str; 7]) -> Self {
        Self {
            workspace_background: colors[0].into(),
            panel_background: colors[1].into(),
            editor_background: colors[2].into(),
            border: colors[3].into(),
            foreground: colors[4].into(),
            muted: colors[5].into(),
            accent: colors[6].into(),
            selection: colors[7].into(),
            diff_removed: colors[8].into(),
            diff_removed_strong: colors[9].into(),
            diff_removed_word: colors[10].into(),
            diff_added: colors[11].into(),
            diff_added_strong: colors[12].into(),
            diff_added_word: colors[13].into(),
            syntax: SyntaxColorsFile {
                comment: syntax[0].into(),
                keyword: syntax[1].into(),
                string: syntax[2].into(),
                number: syntax[3].into(),
                r#type: syntax[4].into(),
                function: syntax[5].into(),
                variable: syntax[6].into(),
            },
        }
    }
}

impl From<EditorSettingsFile> for EditorSettings {
    fn from(value: EditorSettingsFile) -> Self {
        Self {
            version: value.version,
            theme: value.theme,
            default_encoding: value.default_encoding,
            editor: EditorFontSettings {
                font_families: value.editor.font_families,
                font_size: value.editor.font_size,
                line_height: value.editor.line_height,
                minimap: value.editor.minimap,
            },
            large_file: LargeFileSettings {
                max_bytes: value.large_file.max_bytes,
                max_lines: value.large_file.max_lines,
            },
            themes: EditorThemes {
                light: value.themes.light.into(),
                dark: value.themes.dark.into(),
            },
        }
    }
}

impl From<ThemeColorsFile> for EditorThemeColors {
    fn from(value: ThemeColorsFile) -> Self {
        Self {
            workspace_background: value.workspace_background,
            panel_background: value.panel_background,
            editor_background: value.editor_background,
            border: value.border,
            foreground: value.foreground,
            muted: value.muted,
            accent: value.accent,
            selection: value.selection,
            diff_removed: value.diff_removed,
            diff_removed_strong: value.diff_removed_strong,
            diff_removed_word: value.diff_removed_word,
            diff_added: value.diff_added,
            diff_added_strong: value.diff_added_strong,
            diff_added_word: value.diff_added_word,
            syntax: EditorSyntaxColors {
                comment: value.syntax.comment,
                keyword: value.syntax.keyword,
                string: value.syntax.string,
                number: value.syntax.number,
                r#type: value.syntax.r#type,
                function: value.syntax.function,
                variable: value.syntax.variable,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EditorSettingsService;
    use revier_analysis::contracts::{EditorThemeMode, TextEncoding};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn creates_confirmed_default_toml_when_file_is_missing() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("nested/editor.toml");

        let snapshot = EditorSettingsService::load(path.clone()).snapshot();
        let written = fs::read_to_string(&path).unwrap();
        let reloaded = EditorSettingsService::load(path.clone()).snapshot();
        let expected_path = directory.path().join("expected.toml");
        fs::write(&expected_path, valid_toml()).unwrap();
        let expected = EditorSettingsService::load(expected_path).snapshot();

        assert_eq!(snapshot.config_path, path.to_string_lossy());
        assert!(snapshot.warning.is_none());
        assert_eq!(
            serde_json::to_value(&snapshot.settings).unwrap(),
            serde_json::to_value(&reloaded.settings).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&snapshot.settings).unwrap(),
            serde_json::to_value(&expected.settings).unwrap()
        );
        assert_eq!(snapshot.settings.version, 1);
        assert!(matches!(snapshot.settings.theme, EditorThemeMode::System));
        assert!(matches!(
            snapshot.settings.default_encoding,
            TextEncoding::Auto
        ));
        assert_eq!(
            snapshot.settings.editor.font_families,
            [
                "JetBrainsMono Nerd Font Mono",
                "Microsoft YaHei",
                "monospace"
            ]
        );
        assert_eq!(snapshot.settings.editor.font_size, 13);
        assert_eq!(snapshot.settings.editor.line_height, 22);
        assert!(snapshot.settings.editor.minimap);
        assert_eq!(snapshot.settings.large_file.max_bytes, 1_048_576);
        assert_eq!(snapshot.settings.large_file.max_lines, 5_000);
        assert!(written.contains("workspace_background = \"#F4F6F8\""));
        assert!(written.contains("variable = \"#DCE3EC\""));
    }

    #[test]
    fn loads_complete_valid_toml() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("editor.toml");
        fs::write(
            &path,
            valid_toml().replace("theme = \"system\"", "theme = \"dark\""),
        )
        .unwrap();

        let snapshot = EditorSettingsService::load(path).snapshot();

        assert!(snapshot.warning.is_none());
        assert!(matches!(snapshot.settings.theme, EditorThemeMode::Dark));
        assert_eq!(snapshot.settings.editor.font_size, 13);
        assert_eq!(snapshot.settings.large_file.max_lines, 5_000);
        assert_eq!(snapshot.settings.themes.light.syntax.keyword, "#893CAD");
        assert_eq!(snapshot.settings.themes.dark.diff_added_word, "#30664C");
        assert_eq!(snapshot.settings.themes.dark.syntax.function, "#82B7FF");
    }

    #[test]
    fn keeps_broken_file_and_returns_defaults_with_warning() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("editor.toml");
        let original = b"version = [broken\xFF";
        fs::write(&path, original).unwrap();

        let snapshot = EditorSettingsService::load(path.clone()).snapshot();

        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(snapshot.settings.version, 1);
        let warning = snapshot.warning.unwrap();
        assert!(warning.contains(&path.to_string_lossy().to_string()));
        assert!(warning.contains("解析"));
    }

    #[test]
    fn rejects_unknown_theme_and_invalid_hex_color_as_whole_snapshot() {
        for (replacement, expected_error) in [
            (("theme = \"system\"", "theme = \"neon\""), "theme"),
            (
                ("accent = \"#0F766E\"", "accent = \"teal\""),
                "themes.light.accent",
            ),
        ] {
            let directory = tempdir().unwrap();
            let path = directory.path().join("editor.toml");
            fs::write(&path, valid_toml().replace(replacement.0, replacement.1)).unwrap();

            let snapshot = EditorSettingsService::load(path).snapshot();

            assert!(matches!(snapshot.settings.theme, EditorThemeMode::System));
            assert_eq!(snapshot.settings.themes.light.accent, "#0F766E");
            assert_eq!(snapshot.settings.themes.dark.accent, "#2DD4BF");
            assert!(snapshot.warning.unwrap().contains(expected_error));
        }
    }

    fn valid_toml() -> &'static str {
        r##"version = 1
theme = "system"
default_encoding = "auto"

[editor]
font_families = ["JetBrainsMono Nerd Font Mono", "Microsoft YaHei", "monospace"]
font_size = 13
line_height = 22
minimap = true

[large_file]
max_bytes = 1048576
max_lines = 5000

[themes.light]
workspace_background = "#F4F6F8"
panel_background = "#FFFFFF"
editor_background = "#FCFDFE"
border = "#DFE5EC"
foreground = "#273448"
muted = "#768296"
accent = "#0F766E"
selection = "#DCEFEB"
diff_removed = "#FBE7E5"
diff_removed_strong = "#BC3D35"
diff_removed_word = "#F1B9B3"
diff_added = "#E2F3E8"
diff_added_strong = "#26804A"
diff_added_word = "#A9DBBB"

[themes.light.syntax]
comment = "#768296"
keyword = "#893CAD"
string = "#0B7952"
number = "#A05B00"
type = "#0969DA"
function = "#1C63A5"
variable = "#273448"

[themes.dark]
workspace_background = "#111821"
panel_background = "#161F2A"
editor_background = "#101720"
border = "#293645"
foreground = "#DCE3EC"
muted = "#94A1B3"
accent = "#2DD4BF"
selection = "#173C3A"
diff_removed = "#3E262C"
diff_removed_strong = "#E06B63"
diff_removed_word = "#743A43"
diff_added = "#1D3A30"
diff_added_strong = "#5EC58A"
diff_added_word = "#30664C"

[themes.dark.syntax]
comment = "#94A1B3"
keyword = "#D7A0F2"
string = "#8BD7AE"
number = "#EFB875"
type = "#82B7FF"
function = "#82B7FF"
variable = "#DCE3EC"
"##
    }
}
