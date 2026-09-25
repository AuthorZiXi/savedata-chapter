use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const META_FILENAME: &str = "meta.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterMetaEntry {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterMetaFile {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub parts: BTreeMap<u32, ChapterMetaEntry>,
}

fn default_schema_version() -> u32 {
    1
}

/// 读取 meta.json。文件不存在、内容损坏、字段缺失都返回 None，
/// 不影响章节列表本身。
pub fn load(dir: &Path) -> Option<BTreeMap<u32, ChapterMetaEntry>> {
    let path = dir.join(META_FILENAME);
    let text = std::fs::read_to_string(path).ok()?;
    let file: ChapterMetaFile = serde_json::from_str(&text).ok()?;
    Some(file.parts)
}

/// 更新或删除某章备注。备注为空则从映射里移除。
/// 写完后若无任何条目，删除 meta.json，保持目录干净。
pub fn set_note(dir: &Path, n: u32, note: Option<String>) -> std::io::Result<()> {
    let path = dir.join(META_FILENAME);
    let mut file = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<ChapterMetaFile>(&s).ok())
        .unwrap_or_else(|| ChapterMetaFile {
            schema_version: 1,
            parts: BTreeMap::new(),
        });

    match note {
        Some(note) if !note.is_empty() => {
            file.parts.insert(n, ChapterMetaEntry { note: Some(note) });
        }
        _ => {
            file.parts.remove(&n);
        }
    }

    if file.parts.is_empty() {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }

    let json = serde_json::to_string_pretty(&file)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, json)
}

pub fn remove(dir: &Path, n: u32) -> std::io::Result<()> {
    set_note(dir, n, None)
}