use serde::Serialize;
use std::path::Path;

pub const SAVEDATA_DIR: &str = ".savedata";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
    pub hidden: bool,
    pub risk: bool,
}

pub fn list_files(
    dir: &Path,
    hidden_rules: &[String],
    risk_rules: &[String],
) -> std::io::Result<Vec<FileEntry>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if !meta.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == SAVEDATA_DIR {
            continue;
        }
        let hidden = hidden_rules.iter().any(|r| r.eq_ignore_ascii_case(&name));
        let risk = risk_rules.iter().any(|r| r.eq_ignore_ascii_case(&name));
        out.push(FileEntry {
            name,
            size: meta.len(),
            hidden,
            risk,
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}