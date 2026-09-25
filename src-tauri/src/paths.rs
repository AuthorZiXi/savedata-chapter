use crate::config::SavePathEntry;
use serde::Serialize;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathStatus {
    pub id: String,
    pub exists: bool,
    pub is_directory: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RelocateResult {
    Success {
        new_path: PathBuf,
        candidates_tried: Vec<PathBuf>,
    },
    NotFound {
        candidates_tried: Vec<PathBuf>,
    },
}

pub fn new_entry(path: PathBuf, name: Option<String>) -> SavePathEntry {
    SavePathEntry {
        id: Uuid::new_v4().to_string(),
        name,
        path,
    }
}

pub fn validate(entries: &[SavePathEntry]) -> Vec<PathStatus> {
    entries
        .iter()
        .map(|e| PathStatus {
            id: e.id.clone(),
            exists: e.path.exists(),
            is_directory: e.path.is_dir(),
        })
        .collect()
}

/// 把用户选中的目录解释为旧路径的新位置。
///
/// 策略：
/// 1. 直接命中：用户选中的目录本身就是要找的目录。
/// 2. 后缀重建：把 user_selected 的 basename 与旧路径里最后一个同名组件对齐，
///    用对齐点之后的部分拼出新路径。
///
/// 例如：
///   old = C:\Users\me\Documents\game\save
///   user_selected = D:\Documents
///   对齐到 old 中的 "Documents"，重建为 D:\Documents\game\save
pub fn relocate(old_path: &Path, user_selected: &Path) -> RelocateResult {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // 候选 1：直接命中
    candidates.push(user_selected.to_path_buf());

    // 候选 2：后缀重建
    if let Some(user_basename) = user_selected.file_name() {
        let old_normals: Vec<&OsStr> = old_path
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s),
                _ => None,
            })
            .collect();

        // 从末尾往前找最后一个同名组件
        let match_idx = old_normals
            .iter()
            .rposition(|c| *c == user_basename);

        if let Some(idx) = match_idx {
            let mut rebuilt = user_selected.to_path_buf();
            for comp in &old_normals[idx + 1..] {
                rebuilt.push(comp);
            }
            if !candidates.iter().any(|c| c == &rebuilt) {
                candidates.push(rebuilt);
            }
        }
    }

    for candidate in &candidates {
        if candidate.is_dir() {
            return RelocateResult::Success {
                new_path: candidate.clone(),
                candidates_tried: candidates,
            };
        }
    }

    RelocateResult::NotFound {
        candidates_tried: candidates,
    }
}