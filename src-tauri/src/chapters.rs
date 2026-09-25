use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::config::SavePathMode;
use crate::files::SAVEDATA_DIR;
use crate::meta::{self, ChapterMetaEntry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverwriteStrategy {
    Overwrite,
    Skip,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterInfo {
    pub number: u32,
    pub path: PathBuf,
    pub file_count: usize,
    pub meta: Option<ChapterMetaEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadResult {
    pub copied: usize,
    pub overwritten: usize,
    pub skipped: usize,
}

pub fn savedata_dir(save_path: &Path) -> PathBuf {
    save_path.join(SAVEDATA_DIR)
}

pub fn chapter_dir(save_path: &Path, n: u32) -> PathBuf {
    savedata_dir(save_path).join(n.to_string())
}

pub fn list_chapters(save_path: &Path) -> std::io::Result<Vec<ChapterInfo>> {
    let dir = savedata_dir(save_path);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let metas = meta::load(&dir);

    let mut chapters = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(n) = name.parse::<u32>() else { continue };
        let path = entry.path();
        let file_count = count_files(&path).unwrap_or(0);
        let meta = metas.as_ref().and_then(|m| m.get(&n).cloned());
        chapters.push(ChapterInfo {
            number: n,
            path,
            file_count,
            meta,
        });
    }
    chapters.sort_by_key(|c| c.number);
    Ok(chapters)
}

fn count_files(dir: &Path) -> std::io::Result<usize> {
    let mut n = 0;
    for entry in std::fs::read_dir(dir)? {
        if entry?.metadata()?.is_file() {
            n += 1;
        }
    }
    Ok(n)
}

/// 下一个章节号：现有最大号 + 1。允许间隙，不填洞。
pub fn next_number(save_path: &Path) -> u32 {
    list_chapters(save_path)
        .unwrap_or_default()
        .iter()
        .map(|c| c.number)
        .max()
        .unwrap_or(0)
        + 1
}

pub fn create_chapter(
    save_path: &Path,
    files: &[String],
    mode: SavePathMode,
    note: Option<String>,
) -> Result<ChapterInfo, String> {
    if files.is_empty() {
        return Err("没有选中任何文件".into());
    }
    let n = next_number(save_path);
    let target = chapter_dir(save_path, n);
    std::fs::create_dir_all(&target).map_err(|e| format!("创建章节文件夹失败: {}", e))?;

    let mut copied_files = Vec::new();
    for name in files {
        let src = save_path.join(name);
        if !src.is_file() {
            continue;
        }
        let dst = target.join(name);
        let result = match mode {
            SavePathMode::Copy => std::fs::copy(&src, &dst).map(|_| ()),
            SavePathMode::Move => std::fs::rename(&src, &dst),
        };
        result.map_err(|e| format!("处理 {} 失败: {}", name, e))?;
        copied_files.push(name.clone());
    }

    if copied_files.is_empty() {
        return Err("没有可保存的文件".into());
    }

    if let Some(note) = note {
        let dir = savedata_dir(save_path);
        let _ = meta::set_note(&dir, n, Some(note));
    }

    let meta = meta::load(&savedata_dir(save_path))
        .and_then(|m| m.get(&n).cloned());

    Ok(ChapterInfo {
        number: n,
        path: target,
        file_count: copied_files.len(),
        meta,
    })
}

pub fn load_chapter(
    save_path: &Path,
    n: u32,
    strategy: OverwriteStrategy,
) -> Result<LoadResult, String> {
    let src_dir = chapter_dir(save_path, n);
    if !src_dir.is_dir() {
        return Err("章节不存在".into());
    }

    let mut copied = 0;
    let mut overwritten = 0;
    let mut skipped = 0;

    for entry in std::fs::read_dir(&src_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            continue;
        }
        let name = entry.file_name();
        let dst = save_path.join(&name);
        if dst.exists() {
            match strategy {
                OverwriteStrategy::Skip => {
                    skipped += 1;
                    continue;
                }
                OverwriteStrategy::Overwrite => {
                    std::fs::copy(entry.path(), &dst).map_err(|e| e.to_string())?;
                    overwritten += 1;
                }
            }
        } else {
            std::fs::copy(entry.path(), &dst).map_err(|e| e.to_string())?;
            copied += 1;
        }
    }

    Ok(LoadResult {
        copied,
        overwritten,
        skipped,
    })
}

pub fn delete_chapter(save_path: &Path, n: u32) -> Result<(), String> {
    let dir = chapter_dir(save_path, n);
    if !dir.is_dir() {
        return Err("章节不存在".into());
    }
    trash::delete(&dir).map_err(|e| format!("移到回收站失败: {}", e))?;
    let savedata = savedata_dir(save_path);
    let _ = meta::remove(&savedata, n);
    Ok(())
}

pub fn update_note(save_path: &Path, n: u32, note: Option<String>) -> Result<(), String> {
    let dir = savedata_dir(save_path);
    if !dir.is_dir() {
        return Err("章节容器不存在".into());
    }
    meta::set_note(&dir, n, note).map_err(|e| e.to_string())
}