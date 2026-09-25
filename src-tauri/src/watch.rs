use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

pub const EVENT_NAME: &str = "save-path-changed";

#[derive(Default)]
pub struct WatchState {
    watcher: Mutex<Option<RecommendedWatcher>>,
    current: Mutex<Option<PathBuf>>,
}

/// 切换监听的目录。传 None 表示停止监听。
pub fn set_watch(app: AppHandle, state: &WatchState, dir: Option<PathBuf>) -> Result<(), String> {
    // 先停掉旧的
    *state.watcher.lock().unwrap() = None;
    *state.current.lock().unwrap() = None;

    let Some(dir) = dir else { return Ok(()) };
    if !dir.is_dir() {
        return Err("目标不是文件夹".into());
    }

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
        let Ok(event) = res else { return };
        match event.kind {
            EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_) => {}
            _ => return,
        }
        let _ = app.emit(EVENT_NAME, ());
    })
    .map_err(|e| e.to_string())?;

    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;

    let savedata = dir.join(crate::files::SAVEDATA_DIR);
    if savedata.is_dir() {
        watcher
            .watch(&savedata, RecursiveMode::NonRecursive)
            .map_err(|e| e.to_string())?;
    }

    *state.watcher.lock().unwrap() = Some(watcher);
    *state.current.lock().unwrap() = Some(dir);
    Ok(())
}
