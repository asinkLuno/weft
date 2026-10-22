//! YAML 文档边界：数据模型、语法与结构校验、文件监听及变更推送。
//! 保存前通过同一解析流程校验，错误通过 DocumentPayload 返回前端。

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use notify::RecursiveMode;
use notify_debouncer_full::{Debouncer, RecommendedCache, new_debouncer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_log::log::{error, warn};

use crate::weft::entity::{EntityCollection, RawEntity, RawEntityLink};
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::event::{
    self, EventItem, RawEvent, build_event_index, build_events, check as check_event,
};
use crate::weft::narrative::{
    self, Narrative, RawNarrative, check as check_narrative, resolve_narratives,
};
use crate::weft::story::{RawStory, Story};

// YAML 数据模型：顶层键逐个补齐，未知键在反序列化时滤掉。
#[derive(Debug, Serialize)]
pub struct WeftDoc {
    pub story: Story,
    #[serde(flatten)]
    pub entity: EntityCollection,
    // 事件分组：组名 → 按 tick 排序的事件列表
    #[serde(default)]
    pub event: BTreeMap<String, Vec<EventItem>>,
    // 叙事线：组名 → 相对 observer base_time 展开的事件时间轴
    #[serde(default)]
    pub narrative: BTreeMap<String, Narrative>,
    // 运行时填充：所选历法的时间分量数，不来自 YAML
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_components: Option<usize>,
}

#[derive(Deserialize)]
struct RawWeftDoc {
    story: RawStory,
    #[serde(default)]
    entity: HashMap<String, RawEntity>,
    #[serde(default)]
    entity_link: HashMap<String, Vec<RawEntityLink>>,
    #[serde(default)]
    event: HashMap<String, HashMap<String, RawEvent>>,
    #[serde(default)]
    narrative: HashMap<String, RawNarrative>,
}

impl WeftDoc {
    fn from_yaml(
        yaml: &serde_yaml_ng::Value,
        base: &Path,
        gregorian: &Path,
        lang: &str,
    ) -> Result<Self, DocumentError> {
        let raw = RawWeftDoc::deserialize(yaml).map_err(DocumentError::raw)?;
        // 按段检查：story → entity → event → narrative；每段先引用后结构，
        // 前面的段不成立，后面段的问题没有报告价值
        // 历法路径仅在输入构建阶段用于标签上下文，不保存到运行时 Story。
        let date_mode = raw
            .story
            .date_mode
            .clone()
            .filter(|source| !source.trim().is_empty());
        let story = Story::new(raw.story, base, gregorian, lang)?;
        let entity = EntityCollection::new(
            raw.entity,
            raw.entity_link,
            &story.calendar,
            &story.calculable_tag_runtime,
            date_mode.as_deref(),
            lang,
        )?;
        // 标题索引只含名字不含事件体，event 与 narrative 的检查、重写都单独对它做
        let index = build_event_index(&raw.event);
        check_event(&raw.event, entity.as_map(), &index)?;
        check_narrative(&raw.narrative, entity.as_map(), &index)?;
        let narrative = resolve_narratives(raw.narrative, &index);
        let event = build_events(raw.event)?;
        let doc = WeftDoc {
            story,
            entity,
            event,
            narrative,
            calendar_components: None,
        };
        Ok(doc)
    }
}

const EVENT: &str = "document-changed";

#[derive(Default)]
pub struct DocumentState {
    path: Option<PathBuf>,
    // 最近一次由我们写盘的内容，用于抑制自己保存触发的 watcher 回声
    last_written: Option<String>,
    // drop 即停止监听，因此 bind 时直接用新 watcher 覆盖旧值
    _watcher: Option<Debouncer<notify::RecommendedWatcher, RecommendedCache>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPayload {
    doc: Option<Value>,
    content: Option<String>,
    error: Option<DocumentError>,
}

impl DocumentPayload {
    fn ready(doc: Value, content: String) -> Self {
        Self {
            doc: Some(doc),
            content: Some(content),
            error: None,
        }
    }

    // 解析失败时可视情况保留原文：绑定的文件即使有语法错误，编辑器仍要能显示它
    fn failed(error: DocumentError, content: Option<String>) -> Self {
        Self {
            doc: None,
            content,
            error: Some(error),
        }
    }
}

fn parse(app: &AppHandle, content: &str, base: &Path, lang: &str) -> Result<Value, DocumentError> {
    let yaml: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(content).map_err(DocumentError::raw)?;
    // 序列化 weft_doc.rs 声明过的结构：未知键在这里被滤掉
    let gregorian = app
        .path()
        .resolve(
            crate::calendar::GREGORIAN,
            tauri::path::BaseDirectory::Resource,
        )
        .map_err(DocumentError::raw)?;
    let mut doc = WeftDoc::from_yaml(&yaml, base, &gregorian, lang)?;
    // 实体及可计算归属已构建完毕，继续计算事件与叙事线时间。
    let cal = &doc.story.calendar;
    doc.calendar_components = Some(cal.component_count());
    event::apply_times(&mut doc.event, doc.entity.as_map(), cal)?;
    narrative::apply_times(&mut doc.narrative, doc.entity.as_map(), &doc.event, cal)?;
    serde_json::to_value(doc).map_err(DocumentError::raw)
}

fn read_doc(
    app: &AppHandle,
    path: &Path,
    lang: &str,
) -> Result<(String, Result<Value, DocumentError>), DocumentError> {
    let content = fs::read_to_string(path).map_err(DocumentError::raw)?;
    // 语法错误不算绑定失败：正常进入展示页，错误原样带给前端
    let parsed = parse(app, &content, path.parent().unwrap_or(Path::new(".")), lang);
    Ok((content, parsed))
}

// watcher 线程里调用：读文件、回声抑制、把解析结果推给前端
fn push_changed(app: &AppHandle, path: &Path) {
    let Ok(content) = fs::read_to_string(path) else {
        // 读失败但文件还在：写入中途的瞬时状态，等下一个防抖事件；不在了就是被删/移走
        if !path.exists() {
            warn!("bound document deleted or moved: {}", path.display());
            let _ = app.emit(
                EVENT,
                DocumentPayload {
                    doc: None,
                    content: None,
                    error: Some(ValidationError::FileDeleted.into()),
                },
            );
        }
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
    let payload = match parse(
        app,
        &content,
        path.parent().unwrap_or(Path::new(".")),
        &crate::current_lang(app),
    ) {
        Ok(doc) => DocumentPayload::ready(doc, content),
        Err(error) => {
            error!(
                "failed to parse changed document {}: {error}",
                path.display()
            );
            DocumentPayload::failed(error, None)
        }
    };
    let _ = app.emit(EVENT, payload);
}

// 切换应用语言后，立即用同一份文件重新生成日期和可计算标签的展示值。
pub fn refresh_language(app: &AppHandle) {
    let path = {
        let state = app.state::<Mutex<DocumentState>>();
        let st = state.lock().unwrap_or_else(|p| p.into_inner());
        st.path.clone()
    };
    let Some(path) = path else { return };
    let payload = match read_doc(app, &path, &crate::current_lang(app)) {
        Ok((content, Ok(doc))) => DocumentPayload::ready(doc, content),
        Ok((_, Err(error))) | Err(error) => DocumentPayload::failed(error, None),
    };
    let _ = app.emit(EVENT, payload);
}

#[tauri::command]
pub fn bind_document(
    app: AppHandle,
    state: State<'_, Mutex<DocumentState>>,
    path: String,
) -> Result<DocumentPayload, DocumentError> {
    let path = PathBuf::from(path);
    let (content, parsed) =
        read_doc(&app, &path, &crate::current_lang(&app)).inspect_err(|error| {
            error!("failed to read document {}: {error}", path.display());
        })?;

    // 监听父目录再按路径过滤：直接 watch 单文件会漏掉 rename 型写入（我们的原子写就是 rename）
    let target = path.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(300),
        None,
        move |res: Result<Vec<notify_debouncer_full::DebouncedEvent>, Vec<notify::Error>>| {
            let events = match res {
                Ok(events) => events,
                Err(errors) => {
                    warn!(
                        "document watcher failed for {}: {errors:?}",
                        target.display()
                    );
                    return;
                }
            };
            if events.iter().any(|e| e.paths.iter().any(|p| p == &target)) {
                push_changed(&app, &target);
            }
        },
    )
    .map_err(|error| {
        error!("failed to create watcher for {}: {error}", path.display());
        DocumentError::raw(error)
    })?;
    let parent = path.parent().unwrap_or(Path::new("."));
    debouncer
        .watch(parent, RecursiveMode::NonRecursive)
        .map_err(|error| {
            error!("failed to watch directory {}: {error}", parent.display());
            DocumentError::raw(error)
        })?;

    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
    st.path = Some(path.clone());
    st.last_written = None;
    st._watcher = Some(debouncer);

    Ok(match parsed {
        Ok(doc) => DocumentPayload::ready(doc, content),
        Err(error) => {
            error!("failed to parse document {}: {error}", path.display());
            DocumentPayload::failed(error, Some(content))
        }
    })
}

#[tauri::command]
pub fn save_document(
    app: AppHandle,
    state: State<'_, Mutex<DocumentState>>,
    content: String,
) -> Result<(), DocumentError> {
    // 全程持锁：绑定路径在保存期间不能变，watcher 线程最多等这次校验和写盘
    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
    let path = st
        .path
        .clone()
        .ok_or(DocumentError::validation(ValidationError::DocumentNotBound))?;
    // 非法 YAML / 历法错误不落盘；目录取自当前绑定，保存不改路径
    parse(
        &app,
        &content,
        path.parent().unwrap_or(Path::new(".")),
        &crate::current_lang(&app),
    )
    .inspect_err(|error| {
        error!("failed to validate document before saving: {error}");
    })?;
    // 原子写：先写临时文件再 rename，外部 watcher 不会读到半截内容
    let name = path
        .file_name()
        .ok_or(DocumentError::validation(ValidationError::InvalidPath))?;
    let tmp = path.with_file_name(format!(".{}.weft-tmp", name.to_string_lossy()));
    fs::write(&tmp, &content).map_err(|error| {
        error!(
            "failed to write temporary document {}: {error}",
            tmp.display()
        );
        DocumentError::raw(error)
    })?;
    if let Err(e) = fs::rename(&tmp, &path) {
        error!("failed to replace document {}: {e}", path.display());
        let _ = fs::remove_file(&tmp);
        return Err(DocumentError::raw(e));
    }
    // 写成功后才记录回声内容；写失败时下一次 watcher 事件照常处理
    st.last_written = Some(content);
    Ok(())
}

#[tauri::command]
pub fn unbind_document(state: State<'_, Mutex<DocumentState>>) {
    *state.lock().unwrap_or_else(|p| p.into_inner()) = DocumentState::default();
}
