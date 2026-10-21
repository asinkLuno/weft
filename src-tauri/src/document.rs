use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use notify::RecursiveMode;
use notify_debouncer_full::{Debouncer, FileIdMap, new_debouncer};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

const EVENT: &str = "document-changed";

#[derive(Default)]
pub struct DocumentState {
    path: Option<PathBuf>,
    // 最近一次由我们写盘的内容，用于抑制自己保存触发的 watcher 回声
    last_written: Option<String>,
    // drop 即停止监听，因此 bind 时直接用新 watcher 覆盖旧值
    _watcher: Option<Debouncer<notify::RecommendedWatcher, FileIdMap>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPayload {
    doc: Option<Value>,
    content: Option<String>,
    error: Option<String>,
}

fn parse(content: &str) -> Result<Value, String> {
    let yaml: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(content).map_err(|e| e.to_string())?;
    serde_json::to_value(yaml).map_err(|e| e.to_string())
}

fn read_doc(path: &Path) -> Result<(String, Value), String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let doc = parse(&content)?;
    Ok((content, doc))
}

// watcher 线程里调用：读文件、回声抑制、把解析结果推给前端
fn push_changed(app: &AppHandle, path: &Path) {
    // 写入中途的瞬时状态可能读不到，等下一个防抖事件
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    let state = app.state::<Mutex<DocumentState>>();
    {
        let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
        if st.last_written.as_deref() == Some(content.as_str()) {
            st.last_written = None;
            return;
        }
    }
    let payload = match parse(&content) {
        Ok(doc) => DocumentPayload {
            doc: Some(doc),
            content: Some(content),
            error: None,
        },
        Err(error) => DocumentPayload {
            doc: None,
            content: None,
            error: Some(error),
        },
    };
    let _ = app.emit(EVENT, payload);
}

#[tauri::command]
pub fn bind_document(
    app: AppHandle,
    state: State<'_, Mutex<DocumentState>>,
    path: String,
) -> Result<DocumentPayload, String> {
    let path = PathBuf::from(path);
    let (content, doc) = read_doc(&path)?;

    // 监听父目录再按路径过滤：直接 watch 单文件会漏掉 rename 型写入（我们的原子写就是 rename）
    let target = path.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(300),
        None,
        move |res: Result<Vec<notify_debouncer_full::DebouncedEvent>, Vec<notify::Error>>| {
            let Ok(events) = res else { return };
            if events
                .iter()
                .any(|e| e.paths.iter().any(|p| p == &target))
            {
                push_changed(&app, &target);
            }
        },
    )
    .map_err(|e| e.to_string())?;
    let parent = path.parent().unwrap_or(Path::new("."));
    debouncer
        .watch(parent, RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;

    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
    st.path = Some(path);
    st.last_written = None;
    st._watcher = Some(debouncer);

    Ok(DocumentPayload {
        doc: Some(doc),
        content: Some(content),
        error: None,
    })
}

#[tauri::command]
pub fn save_document(state: State<'_, Mutex<DocumentState>>, content: String) -> Result<(), String> {
    parse(&content)?; // 非法 YAML 不落盘
    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
    let path = st.path.clone().ok_or("尚未绑定文档")?;
    st.last_written = Some(content.clone());
    // 原子写：先写临时文件再 rename，外部 watcher 不会读到半截内容
    let name = path.file_name().ok_or("无效路径")?;
    let tmp = path.with_file_name(format!(".{}.weft-tmp", name.to_string_lossy()));
    fs::write(&tmp, &content).map_err(|e| e.to_string())?;
    if let Err(e) = fs::rename(&tmp, &path) {
        st.last_written = None;
        let _ = fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn unbind_document(state: State<'_, Mutex<DocumentState>>) {
    *state.lock().unwrap_or_else(|p| p.into_inner()) = DocumentState::default();
}
