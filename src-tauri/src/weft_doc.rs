//! `.weft` 文档边界：Buffer-First 编译、焦点磁盘检查（Focus Check）与保存冲突守卫（Save Guard）。
//! 内存编辑缓冲区是唯一真相来源，磁盘只作为持久化边界，不再运行后台文件监听线程。

use std::{
    collections::{BTreeMap, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_log::log::error;
use weft_core::calendar::{Calendar, CalendarTick};
use weft_core::compiler::{self, CheckOptions, StoryScriptRefs};
use weft_core::errors::DocumentError;
use weft_core::format::format_weft;
use weft_core::plugin_loader::MAX_SCRIPT_BYTES;
use weft_ls::server::LspServer;

#[derive(Clone, PartialEq, Eq)]
enum ScriptFingerprint {
    Missing,
    Present { len: u64, hash: u64 },
    Oversized { len: u64 },
    Unreadable,
}

struct DocumentSession {
    path: PathBuf,
    parent: PathBuf,
    // 最近一次打开/重载/保存时确认的磁盘原文，用于外部变更探测与保存前乐观锁。
    disk_base: String,
    // 最近一次编译提取到的外部 .rn 脚本指纹，用于在窗口回到焦点时检测脚本改动。
    script_fingerprints: BTreeMap<PathBuf, ScriptFingerprint>,
    // 最近一次成功编译的历法实例，供甘特图按需生成非公历刻度与展示文案。
    calendar: Option<Arc<Calendar>>,
}

#[derive(Deserialize)]
pub struct TickSegmentRequest {
    start: i64,
    end: i64,
    count: usize,
}

#[derive(Default)]
pub struct DocumentState {
    active: Option<DocumentSession>,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CompileResult {
    Ok { doc: Value },
    Invalid,
    Failed { message: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenDocumentResult {
    content: String,
    compile: CompileResult,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DiskFileState {
    Unchanged,
    Modified { content: String },
    Deleted,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInspection {
    file: DiskFileState,
    scripts_changed: bool,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SaveResult {
    Saved {
        content: String,
        compile: CompileResult,
    },
    Conflict {
        #[serde(rename = "diskContent")]
        disk_content: String,
    },
}

fn non_empty_parent(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

fn resolve_document_path(raw: &str) -> Result<(PathBuf, PathBuf), DocumentError> {
    let raw_path = PathBuf::from(raw);
    let file_name = raw_path
        .file_name()
        .ok_or_else(|| DocumentError::raw("invalid path"))?;
    let parent = fs::canonicalize(non_empty_parent(&raw_path)).map_err(DocumentError::raw)?;
    let path = parent.join(file_name);
    Ok((parent, path))
}

fn normalize_script_path(raw: PathBuf) -> PathBuf {
    let Some(file_name) = raw.file_name() else {
        return raw;
    };
    match fs::canonicalize(non_empty_parent(&raw)) {
        Ok(parent) => parent.join(file_name),
        Err(_) => raw,
    }
}

fn snapshot_script(path: &Path) -> ScriptFingerprint {
    let metadata = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return ScriptFingerprint::Missing,
        Err(_) => return ScriptFingerprint::Unreadable,
    };
    if !metadata.is_file() {
        return ScriptFingerprint::Missing;
    }
    let len = metadata.len();
    if len > MAX_SCRIPT_BYTES {
        return ScriptFingerprint::Oversized { len };
    }
    match fs::read(path) {
        Ok(bytes) => {
            let mut hasher = DefaultHasher::new();
            bytes.hash(&mut hasher);
            ScriptFingerprint::Present {
                len,
                hash: hasher.finish(),
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => ScriptFingerprint::Missing,
        Err(_) => ScriptFingerprint::Unreadable,
    }
}

fn snapshot_script_refs(refs: StoryScriptRefs, base: &Path) -> BTreeMap<PathBuf, ScriptFingerprint> {
    let mut fingerprints = BTreeMap::new();
    let mut track = |source: &str| {
        let trimmed = source.trim();
        if trimmed.is_empty() {
            return;
        }
        let path = normalize_script_path(base.join(trimmed));
        fingerprints.entry(path.clone()).or_insert_with(|| snapshot_script(&path));
    };
    if let Some(date_mode) = refs.date_mode.as_deref() {
        track(date_mode);
    }
    for source in &refs.calculable_props {
        track(source);
    }
    fingerprints
}

type CompiledOutcome = (
    Option<BTreeMap<PathBuf, ScriptFingerprint>>,
    Option<Arc<Calendar>>,
    CompileResult,
);

fn compile_text(app: &AppHandle, content: &str, base: &Path) -> CompiledOutcome {
    let lang = crate::current_lang(app);
    let out = compiler::check_document(
        content,
        base,
        &CheckOptions {
            run_scripts: true,
            lang,
        },
    );
    let scripts = out.scripts.map(|r| snapshot_script_refs(r, base));

    if let Some(doc) = out.doc {
        for entity in doc.entity.values() {
            if let Some(avatar) = &entity.avatar
                && let Err(err) = app.asset_protocol_scope().allow_file(avatar)
            {
                error!("failed to allow avatar {avatar}: {err}");
                return (
                    scripts,
                    None,
                    CompileResult::Failed {
                        message: err.to_string(),
                    },
                );
            }
        }
        match serde_json::to_value(&doc) {
            Ok(value) => (
                scripts,
                Some(Arc::new(doc.story.calendar)),
                CompileResult::Ok { doc: value },
            ),
            Err(err) => {
                error!("failed to serialize compiled document: {err}");
                (
                    scripts,
                    None,
                    CompileResult::Failed {
                        message: err.to_string(),
                    },
                )
            }
        }
    } else {
        (scripts, None, CompileResult::Invalid)
    }
}

fn write_atomic(path: &Path, content: &str) -> Result<(), DocumentError> {
    let name = path
        .file_name()
        .ok_or_else(|| DocumentError::raw("invalid path"))?;
    let tmp = path.with_file_name(format!(".{}.weft-tmp", name.to_string_lossy()));
    fs::write(&tmp, content).map_err(|err| {
        error!(
            "failed to write temporary document {}: {err}",
            tmp.display()
        );
        DocumentError::raw(err)
    })?;
    if let Err(err) = fs::rename(&tmp, path) {
        error!("failed to replace document {}: {err}", path.display());
        let _ = fs::remove_file(&tmp);
        return Err(DocumentError::raw(err));
    }
    Ok(())
}

fn update_session_artifacts(
    state: &Mutex<DocumentState>,
    expected_path: &Path,
    scripts: Option<BTreeMap<PathBuf, ScriptFingerprint>>,
    calendar: Option<Arc<Calendar>>,
) {
    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(active) = st.active.as_mut().filter(|s| s.path == expected_path) {
        if let Some(scripts) = scripts {
            active.script_fingerprints = scripts;
        }
        if let Some(calendar) = calendar {
            active.calendar = Some(calendar);
        }
    }
}

#[tauri::command]
pub fn open_document(
    app: AppHandle,
    state: State<'_, Mutex<DocumentState>>,
    path: String,
) -> Result<OpenDocumentResult, DocumentError> {
    let (parent, resolved_path) = resolve_document_path(&path).inspect_err(|err| {
        error!("failed to resolve document path {path}: {err}");
    })?;
    let content = fs::read_to_string(&resolved_path).map_err(|err| {
        error!(
            "failed to read document {}: {err}",
            resolved_path.display()
        );
        DocumentError::raw(err)
    })?;
    let (scripts, calendar, compile) = compile_text(&app, &content, &parent);
    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
    st.active = Some(DocumentSession {
        path: resolved_path,
        parent,
        disk_base: content.clone(),
        script_fingerprints: scripts.unwrap_or_default(),
        calendar,
    });
    Ok(OpenDocumentResult { content, compile })
}

/// 编译已保存文档：用于语言切换或外部 `.rn` 脚本变更后刷新视图。
#[tauri::command]
pub fn compile_document(
    app: AppHandle,
    state: State<'_, Mutex<DocumentState>>,
    content: String,
) -> Result<CompileResult, DocumentError> {
    let (path, parent) = {
        let st = state.lock().unwrap_or_else(|p| p.into_inner());
        let active = st
            .active
            .as_ref()
            .ok_or_else(|| DocumentError::raw("no document is open"))?;
        (active.path.clone(), active.parent.clone())
    };
    let (scripts, calendar, compile) = compile_text(&app, &content, &parent);
    update_session_artifacts(state.inner(), &path, scripts, calendar);
    Ok(compile)
}

/// 进程内 LSP 传输通道：直接投递标准 JSON-RPC 2.0 消息并返回服务端响应/通知列表。
#[tauri::command]
pub fn lsp_send(state: State<'_, Mutex<LspServer>>, message: String) -> Vec<String> {
    let mut server = state.lock().unwrap_or_else(|p| p.into_inner());
    server.handle_json(&message)
}

/// 焦点外部变更探测：只读比对 `.weft` 与外部 `.rn` 脚本，不修改文档会话状态。
#[tauri::command]
pub fn inspect_disk(state: State<'_, Mutex<DocumentState>>) -> Result<DiskInspection, DocumentError> {
    let (path, disk_base, tracked_scripts) = {
        let st = state.lock().unwrap_or_else(|p| p.into_inner());
        let active = st
            .active
            .as_ref()
            .ok_or_else(|| DocumentError::raw("no document is open"))?;
        (
            active.path.clone(),
            active.disk_base.clone(),
            active.script_fingerprints.clone(),
        )
    };

    let file = match fs::read_to_string(&path) {
        Ok(content) if content == disk_base => DiskFileState::Unchanged,
        Ok(content) => DiskFileState::Modified { content },
        Err(err) if err.kind() == io::ErrorKind::NotFound || !path.exists() => {
            DiskFileState::Deleted
        }
        Err(err) => {
            error!("failed to inspect document {}: {err}", path.display());
            return Err(DocumentError::raw(err));
        }
    };

    let scripts_changed = tracked_scripts
        .iter()
        .any(|(script_path, old_fp)| snapshot_script(script_path) != *old_fp);

    Ok(DiskInspection {
        file,
        scripts_changed,
    })
}

/// 显式从磁盘重载：刷新 `disk_base` 并按最新磁盘内容编译。
#[tauri::command]
pub fn reload_document(
    app: AppHandle,
    state: State<'_, Mutex<DocumentState>>,
) -> Result<OpenDocumentResult, DocumentError> {
    let (path, parent) = {
        let st = state.lock().unwrap_or_else(|p| p.into_inner());
        let active = st
            .active
            .as_ref()
            .ok_or_else(|| DocumentError::raw("no document is open"))?;
        (active.path.clone(), active.parent.clone())
    };
    let content = fs::read_to_string(&path).map_err(|err| {
        error!("failed to reload document {}: {err}", path.display());
        DocumentError::raw(err)
    })?;
    let (scripts, calendar, compile) = compile_text(&app, &content, &parent);
    {
        let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(active) = st.active.as_mut().filter(|s| s.path == path) {
            active.disk_base = content.clone();
        }
    }
    update_session_artifacts(state.inner(), &path, scripts, calendar);
    Ok(OpenDocumentResult { content, compile })
}

fn maybe_format_for_save(content: String, format_on_save: bool) -> String {
    if !format_on_save {
        return content;
    }
    match format_weft(&content) {
        Ok(formatted) => formatted,
        Err(_) => content,
    }
}

/// 保存缓冲区到磁盘：默认带乐观锁校验（`force = false`），发现磁盘被外部改过时拒绝静默覆盖；
/// 启用 `formatOnSave` 且无语法错误时，先格式化再落盘并编译。
#[tauri::command]
pub fn save_document(
    app: AppHandle,
    state: State<'_, Mutex<DocumentState>>,
    content: String,
    force: bool,
) -> Result<SaveResult, DocumentError> {
    let format_on_save = crate::current_format_on_save(&app);
    let content = maybe_format_for_save(content, format_on_save);
    let (path, parent) = {
        let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
        let active = st
            .active
            .as_mut()
            .ok_or_else(|| DocumentError::raw("no document is open"))?;

        let mut file_exists = true;
        if !force {
            match fs::read_to_string(&active.path) {
                Ok(disk_content) if disk_content != active.disk_base => {
                    return Ok(SaveResult::Conflict { disk_content });
                }
                Ok(_) => {}
                Err(err)
                    if err.kind() == io::ErrorKind::NotFound || !active.path.exists() =>
                {
                    file_exists = false;
                    // 磁盘文件已不存在：允许写入以重建文件，不会覆盖外部修改。
                }
                Err(err) => {
                    error!(
                        "failed to verify disk state before saving {}: {err}",
                        active.path.display()
                    );
                    return Err(DocumentError::raw(err));
                }
            }
        }

        if force || !file_exists || content != active.disk_base {
            write_atomic(&active.path, &content)?;
            active.disk_base = content.clone();
        }
        (active.path.clone(), active.parent.clone())
    };

    let (scripts, calendar, compile) = compile_text(&app, &content, &parent);
    update_session_artifacts(state.inner(), &path, scripts, calendar);
    Ok(SaveResult::Saved { content, compile })
}

/// 为前端甘特图批量生成非公历时间轴刻度及任意额外 tick 的展示文案。
#[tauri::command]
pub fn calendar_axis_ticks(
    state: State<'_, Mutex<DocumentState>>,
    segments: Vec<TickSegmentRequest>,
    extra_ticks: Vec<i64>,
) -> Result<Vec<CalendarTick>, DocumentError> {
    const MAX_SEGMENTS: usize = 32;
    const MAX_EXTRA_TICKS: usize = 128;

    let calendar = {
        let st = state.lock().unwrap_or_else(|p| p.into_inner());
        st.active.as_ref().and_then(|s| s.calendar.clone())
    };
    let Some(calendar) = calendar else {
        return Ok(Vec::new());
    };
    let mut by_tick: BTreeMap<i64, String> = BTreeMap::new();
    for seg in segments.into_iter().take(MAX_SEGMENTS) {
        for item in calendar.axis_ticks(seg.start, seg.end, seg.count)? {
            by_tick.entry(item.tick).or_insert(item.label);
        }
    }
    let missing_extras: Vec<i64> = extra_ticks
        .into_iter()
        .take(MAX_EXTRA_TICKS)
        .filter(|tick| !by_tick.contains_key(tick))
        .collect();
    for item in calendar.humanize_ticks(&missing_extras)? {
        by_tick.entry(item.tick).or_insert(item.label);
    }
    Ok(by_tick
        .into_iter()
        .map(|(tick, label)| CalendarTick { tick, label })
        .collect())
}

#[tauri::command]
pub fn close_document(state: State<'_, Mutex<DocumentState>>) {
    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
    st.active = None;
}
