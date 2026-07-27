use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
#[cfg(not(test))]
use tauri::Runtime;
#[cfg(not(test))]
use tauri_plugin_log::{
    RotationStrategy, Target, TargetKind, TimezoneStrategy,
};

const DEFAULT_LEVEL: &str = "info";
const DEFAULT_FORMAT: &str = "[${time:yyyy-MM-ddTHH:mm:ss.ms}][${level:short}] ${content}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoggingConfig {
    pub enabled: bool,
    pub level: String,
    pub format: String,
    pub channels: BTreeMap<LogChannel, ChannelConfig>,
}

pub struct LoggingService {
    #[cfg_attr(test, allow(dead_code))]
    config: LoggingConfig,
    warning: Option<String>,
}

impl LoggingService {
    pub fn load(path: PathBuf) -> Self {
        let result = load_logging_config(&path);
        let warning = result.as_ref().err().map(|error| {
            format!("日志配置 {} 无法使用：{error}", path.display())
        });
        Self {
            config: result.unwrap_or_default(),
            warning,
        }
    }

    #[cfg_attr(test, allow(dead_code))]
    pub fn config(&self) -> &LoggingConfig {
        &self.config
    }

    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }

    #[cfg(not(test))]
    pub fn build_plugin<R: Runtime>(
        &self,
    ) -> Result<Option<tauri::plugin::TauriPlugin<R>>, String> {
        build_log_plugin(self.config())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogChannel {
    File,
    Console,
    Webview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelConfig {
    pub enabled: bool,
    pub level: String,
    pub format: String,
    pub options: ChannelOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelOptions {
    File {
        max_size: String,
        rotation_count: u32,
    },
    Console {
        color: bool,
    },
    Webview,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        let mut channels = BTreeMap::new();
        channels.insert(
            LogChannel::File,
            ChannelConfig {
                enabled: false,
                level: DEFAULT_LEVEL.into(),
                format: DEFAULT_FORMAT.into(),
                options: ChannelOptions::File {
                    max_size: "5m".into(),
                    rotation_count: 5,
                },
            },
        );
        channels.insert(
            LogChannel::Console,
            ChannelConfig {
                enabled: true,
                level: DEFAULT_LEVEL.into(),
                format: DEFAULT_FORMAT.into(),
                options: ChannelOptions::Console { color: true },
            },
        );
        channels.insert(
            LogChannel::Webview,
            ChannelConfig {
                enabled: true,
                level: DEFAULT_LEVEL.into(),
                format: DEFAULT_FORMAT.into(),
                options: ChannelOptions::Webview,
            },
        );

        Self {
            enabled: true,
            level: DEFAULT_LEVEL.into(),
            format: DEFAULT_FORMAT.into(),
            channels,
        }
    }
}

pub fn parse_logging_config(text: &str) -> Result<LoggingConfig, String> {
    let file: LoggingFile = toml::from_str(text).map_err(|error| format!("解析失败：{error}"))?;
    file.logging.normalize()
}

pub fn load_logging_config(path: &Path) -> Result<LoggingConfig, String> {
    match fs::read_to_string(path) {
        Ok(text) => parse_logging_config(&text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            write_default_logging_config(path).map_err(|error| error.to_string())?;
            Ok(LoggingConfig::default())
        }
        Err(error) => Err(format!("读取失败：{error}")),
    }
}

#[cfg(not(test))]
pub fn build_log_plugin<R: Runtime>(
    config: &LoggingConfig,
) -> Result<Option<tauri::plugin::TauriPlugin<R>>, String> {
    if !config.enabled {
        return Ok(None);
    }

    let enabled_channels = enabled_channels(config);
    if enabled_channels.is_empty() {
        return Ok(None);
    }

    let mut builder = tauri_plugin_log::Builder::new()
        .clear_targets()
        .level(parse_level_filter(&config.level)?)
        .timezone_strategy(TimezoneStrategy::UseLocal);

    if let Some(file) = config.channels.get(&LogChannel::File) {
        if file.enabled {
            if let ChannelOptions::File {
                max_size,
                rotation_count,
            } = &file.options
            {
                builder = builder
                    .max_file_size(parse_size_bytes(max_size)?)
                    .rotation_strategy(RotationStrategy::KeepSome(*rotation_count as usize));
            }
        }
    }

    for (channel, channel_config) in enabled_channels {
        builder = builder.target(build_target(channel, channel_config)?);
    }

    Ok(Some(builder.build()))
}

#[cfg(not(test))]
fn enabled_channels(config: &LoggingConfig) -> Vec<(LogChannel, &ChannelConfig)> {
    config
        .channels
        .iter()
        .filter_map(|(channel, channel_config)| {
            channel_config
                .enabled
                .then_some((*channel, channel_config))
        })
        .collect()
}

#[cfg(not(test))]
fn build_target(
    channel: LogChannel,
    config: &ChannelConfig,
) -> Result<Target, String> {
    let level = parse_level_filter(&config.level)?;
    let format = config.format.clone();
    let target = match channel {
        LogChannel::File => Target::new(TargetKind::LogDir { file_name: None }),
        LogChannel::Console => Target::new(TargetKind::Stdout),
        LogChannel::Webview => Target::new(TargetKind::Webview),
    };
    Ok(target
        .filter(move |metadata| level_allows(level, metadata.level()))
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{}",
                render_log_format(&format, message, record)
            ));
        }))
}

#[cfg(not(test))]
fn level_allows(filter: log::LevelFilter, level: log::Level) -> bool {
    match filter {
        log::LevelFilter::Off => false,
        log::LevelFilter::Error => level <= log::Level::Error,
        log::LevelFilter::Warn => level <= log::Level::Warn,
        log::LevelFilter::Info => level <= log::Level::Info,
        log::LevelFilter::Debug => level <= log::Level::Debug,
        log::LevelFilter::Trace => level <= log::Level::Trace,
    }
}

#[cfg(not(test))]
fn render_log_format(
    template: &str,
    message: &std::fmt::Arguments<'_>,
    record: &log::Record<'_>,
) -> String {
    let content = message.to_string();
    template
        .replace(
            "${time:yyyy-MM-ddTHH:mm:ss.ms}",
            &chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string(),
        )
        .replace("${level:short}", short_level(record.level()))
        .replace("${level}", record.level().as_str())
        .replace("${target}", record.target())
        .replace("${content}", &content)
}

#[cfg(not(test))]
fn short_level(level: log::Level) -> &'static str {
    match level {
        log::Level::Error => "E",
        log::Level::Warn => "W",
        log::Level::Info => "I",
        log::Level::Debug => "D",
        log::Level::Trace => "T",
    }
}

fn parse_level_filter(value: &str) -> Result<log::LevelFilter, String> {
    match value.to_ascii_lowercase().as_str() {
        "off" => Ok(log::LevelFilter::Off),
        "error" => Ok(log::LevelFilter::Error),
        "warn" | "warning" => Ok(log::LevelFilter::Warn),
        "info" => Ok(log::LevelFilter::Info),
        "debug" => Ok(log::LevelFilter::Debug),
        "trace" => Ok(log::LevelFilter::Trace),
        other => Err(format!("日志级别无效：{other}")),
    }
}

fn parse_size_bytes(value: &str) -> Result<u128, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("日志文件大小不能为空".into());
    }
    let unit_start = trimmed
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(trimmed.len());
    let (number, unit) = trimmed.split_at(unit_start);
    let number = number
        .parse::<u128>()
        .map_err(|error| format!("日志文件大小无效：{error}"))?;
    let multiplier = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" => 1024,
        "m" | "mb" => 1024 * 1024,
        "g" | "gb" => 1024 * 1024 * 1024,
        other => return Err(format!("日志文件大小单位无效：{other}")),
    };
    Ok(number * multiplier)
}

fn write_default_logging_config(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(default_logging_toml().as_bytes())
}

fn default_logging_toml() -> &'static str {
    r#"[logging]
enabled = true
level = "info"
format = "[${time:yyyy-MM-ddTHH:mm:ss.ms}][${level:short}] ${content}"

[logging.channels]
file = false
console = true
webview = true
"#
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoggingFile {
    logging: LoggingSection,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoggingSection {
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default = "default_level")]
    level: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    channels: ChannelMap,
}

impl LoggingSection {
    fn normalize(self) -> Result<LoggingConfig, String> {
        let default = LoggingConfig::default();
        let mut channels = default.channels;

        for config in channels.values_mut() {
            config.enabled = self.enabled && config.enabled;
            config.level = self.level.clone();
            config.format = self.format.clone();
        }

        for (channel, override_value) in self.channels.0 {
            let current = channels
                .remove(&channel)
                .ok_or_else(|| format!("未知日志 channel：{}", channel.as_str()))?;
            let enabled = self.enabled && override_value.enabled.unwrap_or(current.enabled);
            let level = override_value
                .level
                .clone()
                .unwrap_or_else(|| current.level.clone());
            let format = override_value
                .format
                .clone()
                .unwrap_or_else(|| current.format.clone());
            let options = merge_channel_options(channel, current.options, override_value)?;
            channels.insert(
                channel,
                ChannelConfig {
                    enabled,
                    level,
                    format,
                    options,
                },
            );
        }

        if !self.enabled {
            for config in channels.values_mut() {
                config.enabled = false;
            }
        }

        for channel in [LogChannel::File, LogChannel::Console, LogChannel::Webview] {
            channels
                .get_mut(&channel)
                .ok_or_else(|| format!("未知日志 channel：{}", channel.as_str()))?;
        }

        Ok(LoggingConfig {
            enabled: self.enabled,
            level: self.level,
            format: self.format,
            channels,
        })
    }
}

fn merge_channel_options(
    channel: LogChannel,
    current: ChannelOptions,
    override_value: ChannelOverride,
) -> Result<ChannelOptions, String> {
    match (channel, current) {
        (LogChannel::File, ChannelOptions::File { max_size, rotation_count }) => {
            Ok(ChannelOptions::File {
                max_size: override_value.max_size.unwrap_or(max_size),
                rotation_count: override_value.rotation_count.unwrap_or(rotation_count),
            })
        }
        (LogChannel::Console, ChannelOptions::Console { color }) => Ok(ChannelOptions::Console {
            color: override_value.color.unwrap_or(color),
        }),
        (LogChannel::Webview, ChannelOptions::Webview) => Ok(ChannelOptions::Webview),
        _ => Err("日志 channel 配置类型不匹配".into()),
    }
}

#[derive(Debug, Default)]
struct ChannelMap(BTreeMap<LogChannel, ChannelOverride>);

impl<'de> Deserialize<'de> for ChannelMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = BTreeMap::<String, ChannelValue>::deserialize(deserializer)?;
        let mut channels = BTreeMap::new();
        for (name, value) in raw {
            let channel = LogChannel::parse(&name).map_err(serde::de::Error::custom)?;
            channels.insert(channel, value.into_override());
        }
        Ok(Self(channels))
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum ChannelValue {
    Enabled(bool),
    Config(ChannelOverride),
}

impl ChannelValue {
    fn into_override(self) -> ChannelOverride {
        match self {
            Self::Enabled(enabled) => ChannelOverride {
                enabled: Some(enabled),
                ..ChannelOverride::default()
            },
            Self::Config(config) => config,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChannelOverride {
    enabled: Option<bool>,
    level: Option<String>,
    format: Option<String>,
    max_size: Option<String>,
    rotation_count: Option<u32>,
    color: Option<bool>,
}

impl LogChannel {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "file" => Ok(Self::File),
            "console" => Ok(Self::Console),
            "webview" => Ok(Self::Webview),
            other => Err(format!("未知日志 channel：{other}")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Console => "console",
            Self::Webview => "webview",
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_level() -> String {
    DEFAULT_LEVEL.into()
}

fn default_format() -> String {
    DEFAULT_FORMAT.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(config: &LoggingConfig, channel: LogChannel) -> &ChannelConfig {
        config.channels.get(&channel).expect("channel 应存在")
    }

    #[test]
    fn 默认配置启用_console_和_webview_但不启用_file() {
        let config = parse_logging_config("[logging]\n").expect("配置应可解析");

        assert!(config.enabled);
        assert_eq!(config.level, "info");
        assert_eq!(config.format, DEFAULT_FORMAT);
        assert!(!channel(&config, LogChannel::File).enabled);
        assert!(channel(&config, LogChannel::Console).enabled);
        assert!(channel(&config, LogChannel::Webview).enabled);
    }

    #[test]
    fn channel_bool_简写只覆盖_enabled_并继承全局配置() {
        let config = parse_logging_config(
            r#"
[logging]
level = "warn"
format = "global"

[logging.channels]
file = true
console = false
"#,
        )
        .expect("配置应可解析");

        assert!(channel(&config, LogChannel::File).enabled);
        assert_eq!(channel(&config, LogChannel::File).level, "warn");
        assert_eq!(channel(&config, LogChannel::File).format, "global");
        assert!(!channel(&config, LogChannel::Console).enabled);
        assert_eq!(channel(&config, LogChannel::Console).level, "warn");
    }

    #[test]
    fn channel_对象覆盖通用字段和独有字段() {
        let config = parse_logging_config(
            r#"
[logging]
level = "info"
format = "global"

[logging.channels.file]
enabled = true
level = "debug"
format = "file-format"
max_size = "10m"
rotation_count = 9

[logging.channels.console]
enabled = true
color = false
"#,
        )
        .expect("配置应可解析");

        let file = channel(&config, LogChannel::File);
        assert!(file.enabled);
        assert_eq!(file.level, "debug");
        assert_eq!(file.format, "file-format");
        assert_eq!(
            file.options,
            ChannelOptions::File {
                max_size: "10m".into(),
                rotation_count: 9
            }
        );

        let console = channel(&config, LogChannel::Console);
        assert_eq!(console.level, "info");
        assert_eq!(console.format, "global");
        assert_eq!(console.options, ChannelOptions::Console { color: false });
    }

    #[test]
    fn 全局_disabled_会禁用全部_channel() {
        let config = parse_logging_config(
            r#"
[logging]
enabled = false

[logging.channels]
file = true
console = true
webview = true
"#,
        )
        .expect("配置应可解析");

        assert!(!config.enabled);
        assert!(config.channels.values().all(|channel| !channel.enabled));
    }

    #[test]
    fn 拼错_enabled_字段会报错() {
        let error = parse_logging_config(
            r#"
[logging.channels.file]
enbale = true
"#,
        )
        .expect_err("拼错字段不应被接受");

        assert!(error.contains("解析失败"));
    }

    #[test]
    fn 支持解析常用日志级别和文件大小() {
        assert_eq!(parse_level_filter("warning").unwrap(), log::LevelFilter::Warn);
        assert_eq!(parse_size_bytes("5m").unwrap(), 5 * 1024 * 1024);
        assert_eq!(parse_size_bytes("12kb").unwrap(), 12 * 1024);
    }

    #[test]
    fn 首次加载会写入默认日志配置文件() {
        let directory = tempfile::tempdir().expect("临时目录应可创建");
        let path = directory.path().join("logging.toml");

        let config = load_logging_config(&path).expect("默认配置应可加载");

        assert!(path.exists());
        assert_eq!(config, LoggingConfig::default());
        assert!(fs::read_to_string(path)
            .expect("默认配置应可读取")
            .contains("[logging.channels]"));
    }

    #[test]
    fn 全局_disabled_会让所有_channel_归一化为_disabled() {
        let config = parse_logging_config(
            r#"
[logging]
enabled = false
"#,
        )
        .expect("配置应可解析");

        assert!(!config.enabled);
        assert!(config.channels.values().all(|channel| !channel.enabled));
    }
}
