//! `.weft` 文档边界：编译、文件监听、变更推送及编辑器诊断接口。
//! 编译错误由编辑器通过 `check_document` 独立接管，不经由 DocumentPayload 弹窗推送。

use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    },
    thread,
    time::Duration,
};

use notify::{EventKind, RecursiveMode, event::ModifyKind};
use notify_debouncer_full::{DebouncedEvent, Debouncer, RecommendedCache, new_debouncer};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_log::log::{error, warn};

use crate::weft::compiler::{self, StoryScriptRefs};
use crate::weft::errors::{CompileDiagnostic, DocumentError};

const EVENT: &str = "document-changed";
const DEBOUNCE_WINDOW: Duration = Duration::from_millis(300);
const READ_RETRY_ATTEMPTS: usize = 3;
const READ_RETRY_DELAY: Duration = Duration::from_millis(50);

const GATE_INIT: u8 = 0;
const GATE_ACTIVE: u8 = 1;
const GATE_STOPPED: u8 = 2;

const DIRTY_NONE: u8 = 0;
const DIRTY_DOCUMENT: u8 = 1;
const DIRTY_DEPENDENCY: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangeTrigger {
    Document,
    DependencyOrRescan,
}

impl ChangeTrigger {
    fn as_dirty_code(self) -> u8 {
        match self {
            Self::Document => DIRTY_DOCUMENT,
            Self::DependencyOrRescan => DIRTY_DEPENDENCY,
        }
    }

    fn from_dirty_code(code: u8) -> Option<Self> {
        match code {
            DIRTY_DOCUMENT => Some(Self::Document),
            DIRTY_DEPENDENCY => Some(Self::DependencyOrRescan),
            _ => None,
        }
    }
}

/// 绑定目标的规范化路径集合，在 `bind_document` 入口一次性解析完成。
struct DocumentTarget {
    parent: PathBuf,
    path: PathBuf,
    // 若文档本身是符号链接且指向另一路径，则同时跟踪真实目标，避免直接编辑真实文件漏事件。
    real_path: Option<PathBuf>,
}

impl DocumentTarget {
    fn resolve(raw: &str) -> Result<Self, DocumentError> {
        let raw_path = PathBuf::from(raw);
        let file_name = raw_path
            .file_name()
            .ok_or_else(|| DocumentError::raw("invalid path"))?;
        let raw_parent = non_empty_parent(&raw_path);
        let parent = fs::canonicalize(raw_parent).map_err(DocumentError::raw)?;
        let path = parent.join(file_name);
        let real_path = fs::canonicalize(&path).ok().filter(|real| real != &path);
        Ok(Self {
            parent,
            path,
            real_path,
        })
    }
}

/// 纯事件过滤器：由 watcher 回调共享读取，变更依赖脚本时由 `ActiveDocument` 原地同步。
struct WatchFilter {
    doc_path: PathBuf,
    doc_parent: PathBuf,
    doc_real_path: Option<PathBuf>,
    script_paths: HashSet<PathBuf>,
    script_parents: HashSet<PathBuf>,
}

impl WatchFilter {
    fn new(target: &DocumentTarget, script_paths: HashSet<PathBuf>) -> Self {
        let script_parents = script_parent_dirs(&script_paths);
        Self {
            doc_path: target.path.clone(),
            doc_parent: target.parent.clone(),
            doc_real_path: target.real_path.clone(),
            script_paths,
            script_parents,
        }
    }

    fn set_scripts(&mut self, script_paths: HashSet<PathBuf>) {
        self.script_parents = script_parent_dirs(&script_paths);
        self.script_paths = script_paths;
    }

    fn classify(&self, events: &[DebouncedEvent]) -> Option<ChangeTrigger> {
        let mut trigger = None;
        for event in events {
            if event.need_rescan() {
                return Some(ChangeTrigger::DependencyOrRescan);
            }
            if !is_content_event(&event.kind) {
                continue;
            }
            let parent_disrupted = is_parent_disruption(&event.kind);
            for path in &event.paths {
                if self
                    .script_paths
                    .iter()
                    .any(|script| paths_match(path, script))
                    || (parent_disrupted
                        && self
                            .script_parents
                            .iter()
                            .any(|dir| paths_match(path, dir)))
                {
                    return Some(ChangeTrigger::DependencyOrRescan);
                }
                if paths_match(path, &self.doc_path)
                    || self
                        .doc_real_path
                        .as_deref()
                        .is_some_and(|real| paths_match(path, real))
                    || (parent_disrupted && paths_match(path, &self.doc_parent))
                {
                    trigger = Some(ChangeTrigger::Document);
                }
            }
        }
        trigger
    }
}

/// watcher 回调生命周期门控：初始化期间缓存变更、解绑/替换时立即停用旧线程回调。
struct WatcherGate {
    state: AtomicU8,
    dirty: AtomicU8,
}

impl WatcherGate {
    fn new() -> Self {
        Self {
            state: AtomicU8::new(GATE_INIT),
            dirty: AtomicU8::new(DIRTY_NONE),
        }
    }

    fn mark_dirty(&self, trigger: ChangeTrigger) {
        self.dirty.fetch_max(trigger.as_dirty_code(), Ordering::SeqCst);
    }

    fn activate(&self) -> Option<ChangeTrigger> {
        self.state.store(GATE_ACTIVE, Ordering::SeqCst);
        ChangeTrigger::from_dirty_code(self.dirty.swap(DIRTY_NONE, Ordering::SeqCst))
    }

    fn stop(&self) {
        self.state.store(GATE_STOPPED, Ordering::SeqCst);
    }
}

struct ActiveDocument {
    generation: u64,
    // 每次状态提交递增：防止耗时解析（旧事件/旧语言刷新）覆盖更新后的结果。
    seq: u64,
    target: DocumentTarget,
    // 最近一次已同步给前端的 .weft 文本；用于幂等抑制自保存回声与重复事件。
    last_synced: Option<String>,
    watch_filter: Arc<Mutex<WatchFilter>>,
    watched_dirs: HashSet<PathBuf>,
    gate: Arc<WatcherGate>,
    watcher: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
}

impl ActiveDocument {
    fn sync_scripts(&mut self, script_paths: HashSet<PathBuf>) {
        let desired = desired_watch_dirs(&self.target, &script_paths);
        for dir in desired.difference(&self.watched_dirs).cloned().collect::<Vec<_>>() {
            match self.watcher.watch(&dir, RecursiveMode::NonRecursive) {
                Ok(()) => {
                    self.watched_dirs.insert(dir);
                }
                Err(error) => {
                    warn!("failed to watch dependency directory {}: {error}", dir.display());
                }
            }
        }
        let stale: Vec<PathBuf> = self
            .watched_dirs
            .difference(&desired)
            .cloned()
            .collect();
        for dir in stale {
            let _ = self.watcher.unwatch(&dir);
            self.watched_dirs.remove(&dir);
        }
        self.watch_filter
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .set_scripts(script_paths);
    }
}

impl Drop for ActiveDocument {
    fn drop(&mut self) {
        self.gate.stop();
    }
}

#[derive(Default)]
pub struct DocumentState {
    next_generation: u64,
    active: Option<ActiveDocument>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPayload {
    generation: u64,
    doc: Option<Value>,
    content: Option<String>,
}

impl DocumentPayload {
    fn ready(generation: u64, doc: Value, content: String) -> Self {
        Self {
            generation,
            doc: Some(doc),
            content: Some(content),
        }
    }

    // 编译失败或文件缺失时保留原文（若有）：编译错误由编辑器诊断通道单独接管，不再推送到文档状态。
    fn uncompiled(generation: u64, content: Option<String>) -> Self {
        Self {
            generation,
            doc: None,
            content,
        }
    }
}

fn non_empty_parent(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

fn script_parent_dirs(script_paths: &HashSet<PathBuf>) -> HashSet<PathBuf> {
    script_paths
        .iter()
        .filter_map(|path| path.parent().map(Path::to_path_buf))
        .collect()
}

fn desired_watch_dirs(
    target: &DocumentTarget,
    script_paths: &HashSet<PathBuf>,
) -> HashSet<PathBuf> {
    let mut dirs = script_parent_dirs(script_paths);
    dirs.insert(target.parent.clone());
    if let Some(real) = &target.real_path
        && let Some(parent) = real.parent()
    {
        dirs.insert(parent.to_path_buf());
    }
    dirs
}

fn resolve_script_watch_paths(raw: &Path, out: &mut HashSet<PathBuf>) {
    let Some(file_name) = raw.file_name() else {
        return;
    };
    let Ok(parent) = fs::canonicalize(non_empty_parent(raw)) else {
        return;
    };
    let resolved = parent.join(file_name);
    if let Ok(real) = fs::canonicalize(&resolved)
        && real != resolved
    {
        out.insert(real);
    }
    out.insert(resolved);
}

fn resolve_script_refs(refs: StoryScriptRefs, base: &Path) -> HashSet<PathBuf> {
    let mut paths = HashSet::new();
    if let Some(date_mode) = refs
        .date_mode
        .as_deref()
        .map(str::trim)
        .filter(|source| !source.is_empty())
    {
        resolve_script_watch_paths(&base.join(date_mode), &mut paths);
    }
    for source in refs.calculable_props {
        let trimmed = source.trim();
        if !trimmed.is_empty() {
            resolve_script_watch_paths(&base.join(trimmed), &mut paths);
        }
    }
    paths
}

fn is_content_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Any
            | EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(
                ModifyKind::Any | ModifyKind::Data(_) | ModifyKind::Name(_) | ModifyKind::Other
            )
    )
}

fn is_parent_disruption(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
    )
}

fn paths_match(event_path: &Path, target: &Path) -> bool {
    #[cfg(windows)]
    {
        event_path
            .to_string_lossy()
            .eq_ignore_ascii_case(&target.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        event_path == target
    }
}

fn is_transient_read_error(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::PermissionDenied || error.raw_os_error() == Some(32)
}

enum ReadOutcome {
    Ready(String),
    Deleted,
    Error(io::Error),
}

fn read_document_text(
    path: &Path,
    state: &Mutex<DocumentState>,
    generation: u64,
) -> Option<ReadOutcome> {
    for attempt in 0..READ_RETRY_ATTEMPTS {
        match fs::read_to_string(path) {
            Ok(content) => return Some(ReadOutcome::Ready(content)),
            Err(error) if error.kind() == io::ErrorKind::NotFound || !path.exists() => {
                return Some(ReadOutcome::Deleted);
            }
            Err(error) if is_transient_read_error(&error) && attempt + 1 < READ_RETRY_ATTEMPTS => {
                thread::sleep(READ_RETRY_DELAY);
                let still_active = state
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .active
                    .as_ref()
                    .is_some_and(|active| active.generation == generation);
                if !still_active {
                    return None;
                }
            }
            Err(error) => return Some(ReadOutcome::Error(error)),
        }
    }
    None
}

fn resolve_gregorian(app: &AppHandle) -> Result<PathBuf, DocumentError> {
    app.path()
        .resolve(
            crate::calendar::GREGORIAN,
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(DocumentError::raw)
}

fn parse(
    app: &AppHandle,
    content: &str,
    base: &Path,
    lang: &str,
) -> (Option<HashSet<PathBuf>>, Result<Value, DocumentError>) {
    let gregorian = match resolve_gregorian(app) {
        Ok(path) => path,
        Err(err) => return (None, Err(err)),
    };
    let (refs, compiled) = compiler::compile_with_scripts(content, base, &gregorian, lang);
    let scripts = refs.map(|r| resolve_script_refs(r, base));
    let result = compiled.and_then(|doc| {
        for entity in doc.entity.values() {
            if let Some(avatar) = &entity.avatar {
                app.asset_protocol_scope()
                    .allow_file(avatar)
                    .map_err(DocumentError::raw)?;
            }
        }
        serde_json::to_value(doc).map_err(DocumentError::raw)
    });
    (scripts, result)
}

fn commit_and_emit(
    app: &AppHandle,
    active: &mut ActiveDocument,
    last_synced: Option<String>,
    payload: DocumentPayload,
) {
    active.seq += 1;
    active.last_synced = last_synced;
    let _ = app.emit(EVENT, payload);
}

// watcher 线程里调用：按会话代数与序号校验、幂等去重、解析并把结果推给前端
fn push_changed(app: &AppHandle, generation: u64, trigger: ChangeTrigger) {
    let state = app.state::<Mutex<DocumentState>>();
    let (path, parent, start_seq) = {
        let st = state.lock().unwrap_or_else(|p| p.into_inner());
        let Some(active) = st
            .active
            .as_ref()
            .filter(|active| active.generation == generation)
        else {
            return;
        };
        (
            active.target.path.clone(),
            active.target.parent.clone(),
            active.seq,
        )
    };

    let Some(outcome) = read_document_text(&path, state.inner(), generation) else {
        return;
    };
    let content = match outcome {
        ReadOutcome::Ready(content) => content,
        ReadOutcome::Deleted => {
            let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(active) = st
                .active
                .as_mut()
                .filter(|active| active.generation == generation && active.seq == start_seq)
            {
                warn!("bound document deleted or moved: {}", path.display());
                commit_and_emit(
                    app,
                    active,
                    None,
                    DocumentPayload::uncompiled(generation, None),
                );
            }
            return;
        }
        ReadOutcome::Error(err) => {
            let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(active) = st
                .active
                .as_mut()
                .filter(|active| active.generation == generation && active.seq == start_seq)
            {
                error!("failed to read changed document {}: {err}", path.display());
                commit_and_emit(
                    app,
                    active,
                    None,
                    DocumentPayload::uncompiled(generation, None),
                );
            }
            return;
        }
    };

    {
        let st = state.lock().unwrap_or_else(|p| p.into_inner());
        let Some(active) = st
            .active
            .as_ref()
            .filter(|active| active.generation == generation && active.seq == start_seq)
        else {
            return;
        };
        if trigger == ChangeTrigger::Document
            && active.last_synced.as_deref() == Some(content.as_str())
        {
            return;
        }
    }

    let (scripts, parsed) = parse(app, &content, &parent, &crate::current_lang(app));
    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
    let Some(active) = st
        .active
        .as_mut()
        .filter(|active| active.generation == generation && active.seq == start_seq)
    else {
        return;
    };
    if let Some(scripts) = scripts {
        active.sync_scripts(scripts);
    }
    match parsed {
        Ok(doc) => commit_and_emit(
            app,
            active,
            Some(content.clone()),
            DocumentPayload::ready(generation, doc, content),
        ),
        Err(err) => {
            error!("failed to parse changed document {}: {err}", path.display());
            commit_and_emit(
                app,
                active,
                None,
                DocumentPayload::uncompiled(generation, Some(content)),
            );
        }
    }
}

// 切换应用语言后，立即用同一份文件重新生成日期和可计算属性的展示值。
pub fn refresh_language(app: &AppHandle) {
    let state = app.state::<Mutex<DocumentState>>();
    let (generation, my_seq, path, parent) = {
        let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
        let Some(active) = st.active.as_mut() else {
            return;
        };
        active.seq += 1;
        (
            active.generation,
            active.seq,
            active.target.path.clone(),
            active.target.parent.clone(),
        )
    };
    let result = fs::read_to_string(&path)
        .map_err(DocumentError::raw)
        .map(|content| {
            let (scripts, parsed) = parse(app, &content, &parent, &crate::current_lang(app));
            (content, scripts, parsed)
        });

    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
    let Some(active) = st
        .active
        .as_mut()
        .filter(|active| active.generation == generation && active.seq == my_seq)
    else {
        return;
    };
    match result {
        Ok((content, scripts, parsed)) => {
            if let Some(scripts) = scripts {
                active.sync_scripts(scripts);
            }
            match parsed {
                Ok(doc) => {
                    active.last_synced = Some(content.clone());
                    let _ = app.emit(EVENT, DocumentPayload::ready(generation, doc, content));
                }
                Err(err) => {
                    error!("failed to refresh document {}: {err}", path.display());
                    active.last_synced = None;
                    let _ = app.emit(EVENT, DocumentPayload::uncompiled(generation, Some(content)));
                }
            }
        }
        Err(err) => {
            error!("failed to read document on language refresh: {err}");
            active.last_synced = None;
            let _ = app.emit(EVENT, DocumentPayload::uncompiled(generation, None));
        }
    }
}

#[tauri::command]
pub fn bind_document(
    app: AppHandle,
    state: State<'_, Mutex<DocumentState>>,
    path: String,
) -> Result<DocumentPayload, DocumentError> {
    let target = DocumentTarget::resolve(&path).inspect_err(|err| {
        error!("failed to resolve document path {path}: {err}");
    })?;
    let generation = {
        let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
        st.next_generation += 1;
        st.next_generation
    };

    let watch_filter = Arc::new(Mutex::new(WatchFilter::new(&target, HashSet::new())));
    let gate = Arc::new(WatcherGate::new());

    // 先建 watcher 再读盘，消除初次绑定到监听生效之间的漏事件窗口；
    // 监听父目录而非单文件，以覆盖原子 rename 写入，并同步监听引用的 .rn 脚本所在目录。
    let cb_app = app.clone();
    let cb_filter = Arc::clone(&watch_filter);
    let cb_gate = Arc::clone(&gate);
    let cb_path = target.path.clone();
    let mut debouncer = new_debouncer(
        DEBOUNCE_WINDOW,
        None,
        move |res: Result<Vec<DebouncedEvent>, Vec<notify::Error>>| {
            if cb_gate.state.load(Ordering::SeqCst) == GATE_STOPPED {
                return;
            }
            let events = match res {
                Ok(events) => events,
                Err(errors) => {
                    warn!(
                        "document watcher failed for {}: {errors:?}",
                        cb_path.display()
                    );
                    return;
                }
            };
            let Some(trigger) = cb_filter
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .classify(&events)
            else {
                return;
            };
            match cb_gate.state.load(Ordering::SeqCst) {
                GATE_STOPPED => {}
                GATE_INIT => cb_gate.mark_dirty(trigger),
                _ => push_changed(&cb_app, generation, trigger),
            }
        },
    )
    .map_err(|err| {
        error!(
            "failed to create watcher for {}: {err}",
            target.path.display()
        );
        DocumentError::raw(err)
    })?;

    let mut watched_dirs = HashSet::new();
    debouncer
        .watch(&target.parent, RecursiveMode::NonRecursive)
        .map_err(|err| {
            error!(
                "failed to watch directory {}: {err}",
                target.parent.display()
            );
            DocumentError::raw(err)
        })?;
    watched_dirs.insert(target.parent.clone());

    let content = fs::read_to_string(&target.path).map_err(|err| {
        error!("failed to read document {}: {err}", target.path.display());
        DocumentError::raw(err)
    })?;
    // 语法/脚本错误不算绑定失败：仍进入工作台，编译诊断由编辑器通道单独获取
    let (scripts, parsed) = parse(&app, &content, &target.parent, &crate::current_lang(&app));
    let initial_scripts = scripts.unwrap_or_default();
    for dir in desired_watch_dirs(&target, &initial_scripts) {
        if watched_dirs.contains(&dir) {
            continue;
        }
        match debouncer.watch(&dir, RecursiveMode::NonRecursive) {
            Ok(()) => {
                watched_dirs.insert(dir);
            }
            Err(err) => {
                warn!("failed to watch dependency directory {}: {err}", dir.display());
            }
        }
    }
    watch_filter
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .set_scripts(initial_scripts);

    let last_synced = parsed.is_ok().then(|| content.clone());
    let pending_trigger = {
        let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
        let active = ActiveDocument {
            generation,
            seq: 0,
            target,
            last_synced,
            watch_filter,
            watched_dirs,
            gate: Arc::clone(&gate),
            watcher: debouncer,
        };
        st.active = Some(active);
        gate.activate()
    };

    if let Some(trigger) = pending_trigger {
        let retry_app = app.clone();
        thread::spawn(move || push_changed(&retry_app, generation, trigger));
    }

    Ok(match parsed {
        Ok(doc) => DocumentPayload::ready(generation, doc, content),
        Err(err) => {
            error!("failed to parse document {path}: {err}");
            DocumentPayload::uncompiled(generation, Some(content))
        }
    })
}

/// 编辑器实时编译诊断入口：仅在内存中编译并返回单条诊断，不写盘、不改变文档状态。
#[tauri::command]
pub fn check_document(
    app: AppHandle,
    state: State<'_, Mutex<DocumentState>>,
    content: String,
) -> Option<CompileDiagnostic> {
    let parent = {
        let st = state.lock().unwrap_or_else(|p| p.into_inner());
        st.active.as_ref()?.target.parent.clone()
    };
    let gregorian = match resolve_gregorian(&app) {
        Ok(path) => path,
        Err(err) => {
            error!("failed to resolve gregorian script: {err}");
            return None;
        }
    };
    match compiler::compile_with_scripts(&content, &parent, &gregorian, &crate::current_lang(&app))
        .1
    {
        Ok(_) => None,
        Err(DocumentError::Validation { error, span }) => Some(CompileDiagnostic { error, span }),
        Err(DocumentError::Raw { message }) => {
            error!("check_document failed: {message}");
            None
        }
    }
}

#[tauri::command]
pub fn save_document(
    app: AppHandle,
    state: State<'_, Mutex<DocumentState>>,
    content: String,
) -> Result<DocumentPayload, DocumentError> {
    // 全程持锁：绑定路径在保存期间不能变，watcher 线程最多等这次写盘和编译
    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
    let active = st
        .active
        .as_mut()
        .ok_or_else(|| DocumentError::raw("no document is bound"))?;
    let path = active.target.path.clone();
    let parent = active.target.parent.clone();
    // 原子写：允许带编译错误落盘（与常规代码编辑器一致），外部 watcher 不会读到半截内容
    let name = path
        .file_name()
        .ok_or_else(|| DocumentError::raw("invalid path"))?;
    let tmp = path.with_file_name(format!(".{}.weft-tmp", name.to_string_lossy()));
    fs::write(&tmp, &content).map_err(|err| {
        error!(
            "failed to write temporary document {}: {err}",
            tmp.display()
        );
        DocumentError::raw(err)
    })?;
    if let Err(err) = fs::rename(&tmp, &path) {
        error!("failed to replace document {}: {err}", path.display());
        let _ = fs::remove_file(&tmp);
        return Err(DocumentError::raw(err));
    }
    let (scripts, parsed) = parse(&app, &content, &parent, &crate::current_lang(&app));
    active.seq += 1;
    active.last_synced = Some(content.clone());
    if let Some(scripts) = scripts {
        active.sync_scripts(scripts);
    }
    Ok(match parsed {
        Ok(doc) => DocumentPayload::ready(active.generation, doc, content),
        Err(err) => {
            error!("saved document has compile errors {}: {err}", path.display());
            DocumentPayload::uncompiled(active.generation, Some(content))
        }
    })
}

#[tauri::command]
pub fn unbind_document(state: State<'_, Mutex<DocumentState>>) {
    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
    st.active = None;
}
