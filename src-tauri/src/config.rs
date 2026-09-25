use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub schema_version: u32,
    pub theme: Theme,
    pub save_path_mode: SavePathMode,
    pub show_hidden_entries: bool,
    pub exclude_rules: ExcludeRules,
    pub save_paths: Vec<SavePathEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SavePathMode {
    Copy,
    Move,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcludeRules {
    pub hidden: Vec<String>,
    pub risk: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePathEntry {
    pub id: String,
    pub name: Option<String>,
    pub path: PathBuf,
}

pub const DEFAULT_HIDDEN: &[&str] = &[
    "datasc.ksd",
    "datasc~.ksd",
    "datasu.ksd",
    "datasu~.ksd",
    "savecheck",
    "data_anchor.ksd",
    "system.dat",
    "krenvprf.kep",
    "krkr.console.log",
    "systemdata.dat",
];

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            theme: Theme::System,
            save_path_mode: SavePathMode::Move,
            show_hidden_entries: false,
            exclude_rules: ExcludeRules::default(),
            save_paths: Vec::new(),
        }
    }
}

impl Default for ExcludeRules {
    fn default() -> Self {
        Self {
            hidden: DEFAULT_HIDDEN.iter().map(|s| s.to_string()).collect(),
            risk: Vec::new(),
        }
    }
}

const CONFIG_FILENAME: &str = "config.json";
const FALLBACK_DIRNAME: &str = "savedata-chapter";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialize error: {0}")]
    Json(#[from] serde_json::Error),
}

/// 程序目录可写就写程序目录，否则写 %AppData%\savedata-chapter\
pub fn config_dir() -> PathBuf {
    if let Some(dir) = program_dir() {
        if is_writable(&dir) {
            return dir;
        }
    }
    fallback_dir()
}

pub fn config_path() -> PathBuf {
    config_dir().join(CONFIG_FILENAME)
}

fn program_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

fn fallback_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join(FALLBACK_DIRNAME);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn is_writable(dir: &Path) -> bool {
    let test = dir.join(".savedata-chapter-writetest");
    match std::fs::File::create(&test) {
        Ok(_) => {
            let _ = std::fs::remove_file(&test);
            true
        }
        Err(_) => false,
    }
}

/// 读取配置。文件不存在或解析失败时返回默认配置，不抛错。
pub fn load() -> AppConfig {
    let path = config_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return AppConfig::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save(config: &AppConfig) -> Result<(), ConfigError> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(&path, json)?;
    Ok(())
}

pub fn to_json(config: &AppConfig) -> Result<String, ConfigError> {
    Ok(serde_json::to_string_pretty(config)?)
}

pub fn from_json(text: &str) -> Result<AppConfig, ConfigError> {
    Ok(serde_json::from_str(text)?)
}
