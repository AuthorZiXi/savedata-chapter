mod chapters;
mod config;
mod files;
mod meta;
mod paths;
mod watch;

use chapters::{ChapterInfo, LoadResult, OverwriteStrategy};
use config::{AppConfig, SavePathEntry, SavePathMode};
use files::FileEntry;
use paths::{PathStatus, RelocateResult};
use std::path::PathBuf;

#[tauri::command]
fn load_config() -> AppConfig {
    config::load()
}

#[tauri::command]
fn save_config(config: AppConfig) -> Result<(), String> {
    config::save(&config).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_save_path(path: PathBuf, name: Option<String>) -> Result<SavePathEntry, String> {
    if !path.is_dir() {
        return Err("目标不是文件夹".into());
    }
    if path
        .file_name()
        .and_then(|n| n.to_str())
        .map_or(false, |n| n == files::SAVEDATA_DIR)
    {
        return Err("不能注册 .savedata 文件夹".into());
    }

    let mut cfg = config::load();
    if cfg.save_paths.iter().any(|e| e.path == path) {
        return Err("该路径已在列表中".into());
    }

    let entry = paths::new_entry(path, name);
    cfg.save_paths.push(entry.clone());
    config::save(&cfg).map_err(|e| e.to_string())?;
    Ok(entry)
}

#[tauri::command]
fn remove_save_path(id: String) -> Result<(), String> {
    let mut cfg = config::load();
    cfg.save_paths.retain(|e| e.id != id);
    config::save(&cfg).map_err(|e| e.to_string())
}

#[tauri::command]
fn validate_save_paths() -> Vec<PathStatus> {
    let cfg = config::load();
    paths::validate(&cfg.save_paths)
}

#[tauri::command]
fn relocate_save_path(id: String, user_selected: PathBuf) -> Result<RelocateResult, String> {
    let mut cfg = config::load();
    let entry = cfg
        .save_paths
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or_else(|| "找不到该路径记录".to_string())?;

    let old = entry.path.clone();
    let result = paths::relocate(&old, &user_selected);

    if let RelocateResult::Success { new_path, .. } = &result {
        entry.path = new_path.clone();
        config::save(&cfg).map_err(|e| e.to_string())?;
    }

    Ok(result)
}

/// 从配置里查路径，并确认它当前是个有效目录。
fn resolve_dir(id: &str) -> Result<PathBuf, String> {
    let cfg = config::load();
    let entry = cfg
        .save_paths
        .iter()
        .find(|e| e.id == id)
        .ok_or_else(|| "路径不存在".to_string())?;
    if !entry.path.is_dir() {
        return Err("路径已失效".into());
    }
    Ok(entry.path.clone())
}

#[tauri::command]
fn list_save_files(id: String) -> Result<Vec<FileEntry>, String> {
    let dir = resolve_dir(&id)?;
    let cfg = config::load();
    files::list_files(&dir, &cfg.exclude_rules.hidden, &cfg.exclude_rules.risk)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_chapters(id: String) -> Result<Vec<ChapterInfo>, String> {
    let dir = resolve_dir(&id)?;
    chapters::list_chapters(&dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_chapter(
    id: String,
    files: Vec<String>,
    mode: SavePathMode,
    note: Option<String>,
) -> Result<ChapterInfo, String> {
    let dir = resolve_dir(&id)?;
    chapters::create_chapter(&dir, &files, mode, note)
}

#[tauri::command]
fn load_chapter(
    id: String,
    number: u32,
    strategy: OverwriteStrategy,
) -> Result<LoadResult, String> {
    let dir = resolve_dir(&id)?;
    chapters::load_chapter(&dir, number, strategy)
}

#[tauri::command]
fn delete_chapter(id: String, number: u32) -> Result<(), String> {
    let dir = resolve_dir(&id)?;
    chapters::delete_chapter(&dir, number)
}

#[tauri::command]
fn update_chapter_note(id: String, number: u32, note: Option<String>) -> Result<(), String> {
    let dir = resolve_dir(&id)?;
    chapters::update_note(&dir, number, note)
}

#[tauri::command]
fn trash_files(id: String, files: Vec<String>) -> Result<(), String> {
    let dir = resolve_dir(&id)?;
    for name in files {
        let path = dir.join(&name);
        if path.is_file() {
            trash::delete(&path).map_err(|e| format!("删除 {} 失败: {}", name, e))?;
        }
    }
    Ok(())
}

#[tauri::command]
fn watch_save_path(
    app: tauri::AppHandle,
    state: tauri::State<watch::WatchState>,
    id: Option<String>,
) -> Result<(), String> {
    let dir = match id {
        Some(id) => Some(resolve_dir(&id)?),
        None => None,
    };
    watch::set_watch(app, &state, dir)
}

#[tauri::command]
fn reveal_in_explorer(path: PathBuf, select: Option<PathBuf>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        let mut cmd = Command::new("explorer");
        match select {
            Some(sel) => {
                cmd.raw_arg(format!("/select,\"{}\"", sel.display()));
            }
            None => {
                cmd.arg(path.as_os_str());
            }
        }
        cmd.spawn().map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = select;
        tauri_plugin_opener::open_path(path.to_string_lossy().to_string(), None::<&str>)
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn export_config(dest: PathBuf) -> Result<(), String> {
    let cfg = config::load();
    let json = config::to_json(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(&dest, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn import_config(src: PathBuf) -> Result<AppConfig, String> {
    let text = std::fs::read_to_string(&src).map_err(|e| e.to_string())?;
    let cfg = config::from_json(&text).map_err(|e| format!("配置格式错误: {}", e))?;
    config::save(&cfg).map_err(|e| e.to_string())?;
    Ok(cfg)
}

#[tauri::command]
fn open_config_dir() -> Result<(), String> {
    let dir = config::config_dir();
    if !dir.is_dir() {
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("explorer")
            .arg(dir.as_os_str())
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        tauri_plugin_opener::open_path(dir.to_string_lossy().to_string(), None::<&str>)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(watch::WatchState::default())
        .invoke_handler(tauri::generate_handler![
            load_config,
            save_config,
            add_save_path,
            remove_save_path,
            validate_save_paths,
            relocate_save_path,
            list_save_files,
            list_chapters,
            create_chapter,
            load_chapter,
            delete_chapter,
            update_chapter_note,
            trash_files,
            watch_save_path,
            reveal_in_explorer,
            export_config,
            import_config,
            open_config_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
