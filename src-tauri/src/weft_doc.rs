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

use crate::calculable_tag::CalculableTagRuntime;
use crate::calendar::Calendar;
use crate::weft::entity::{self, Entity, EntityLink, check as check_entity};
use crate::weft::errors::{DocumentError, ValidationError};
use crate::weft::event::{
    self, EventItem, RawEvent, build_event_index, build_events, check as check_event,
};
use crate::weft::narrative::{
    self, Narrative, RawNarrative, check as check_narrative, resolve_narratives,
};
use crate::weft::plugin::CalendarSpec;
use crate::weft::story::{Story, StoryPlugins};

// YAML 数据模型：顶层键逐个补齐，未知键在反序列化时滤掉。
#[derive(Debug, Serialize)]
pub struct WeftDoc {
    pub story: Story,
    pub entity: HashMap<String, Entity>,
    // 关系分组：组名 → 关系对列表
    #[serde(default)]
    pub entity_link: HashMap<String, Vec<EntityLink>>,
    // 可计算标签注册表：注册名 → Rhai 脚本清单，按实体 base_time 计算归属
    #[serde(default)]
    pub calculable_tag: HashMap<String, CalendarSpec>,
    // 事件分组：组名 → 按 tick 排序的事件列表
    #[serde(default)]
    pub event: BTreeMap<String, Vec<EventItem>>,
    // 叙事线：组名 → 相对 observer base_time 展开的事件时间轴
    #[serde(default)]
    pub narrative: BTreeMap<String, Narrative>,
    // 运行时填充：所选历法的时间分量数，不来自 YAML
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_components: Option<usize>,
    // 当前语言的动态展示信息，由历法 serialize 生成。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calendar_display: Option<Value>,
}

#[derive(Deserialize)]
struct RawWeftDoc {
    story: Story,
    #[serde(default)]
    entity: HashMap<String, Entity>,
    #[serde(default)]
    entity_link: HashMap<String, Vec<EntityLink>>,
    #[serde(default)]
    event: HashMap<String, HashMap<String, RawEvent>>,
    #[serde(default)]
    narrative: HashMap<String, RawNarrative>,
    #[serde(default)]
    calculable_tag: HashMap<String, CalendarSpec>,
}

impl WeftDoc {
    fn from_yaml(
        yaml: &serde_yaml_ng::Value,
        base: &Path,
        gregorian: &Path,
        lang: &str,
    ) -> Result<(Self, StoryPlugins), DocumentError> {
        let raw = RawWeftDoc::deserialize(yaml).map_err(DocumentError::raw)?;
        // 按段检查：story → entity → event → narrative；每段先引用后结构，
        // 前面的段不成立，后面段的问题没有报告价值
        let plugins = raw
            .story
            .load_plugins(&raw.calculable_tag, base, gregorian, lang)?;
        check_entity(&raw.entity, &raw.entity_link)?;
        // 标题索引只含名字不含事件体，event 与 narrative 的检查、重写都单独对它做
        let index = build_event_index(&raw.event);
        check_event(&raw.event, &raw.entity, &index)?;
        check_narrative(&raw.narrative, &raw.entity, &index)?;
        let narrative = resolve_narratives(raw.narrative, &index);
        let event = build_events(raw.event)?;
        let doc = WeftDoc {
            story: raw.story,
            entity: raw.entity,
            entity_link: raw.entity_link,
            event,
            narrative,
            calculable_tag: raw.calculable_tag,
            calendar_components: None,
            calendar_display: None,
        };
        Ok((doc, plugins))
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
    let (mut doc, plugins) = WeftDoc::from_yaml(&yaml, base, &gregorian, lang)?;
    // 插件已在 story 阶段加载完毕，按所选历法解析所有时间。
    let mode = doc.story.date_mode().map(str::to_owned);
    let cal = &plugins.calendar;
    doc.calendar_components = Some(cal.component_count());
    doc.calendar_display = Some(cal.serialize(lang)?);
    entity::apply_times(&mut doc.entity, cal)?;
    apply_calculable_tags(
        &mut doc,
        &plugins.calculable_tags,
        mode.as_deref(),
        lang,
        cal,
    )?;
    event::apply_times(&mut doc.event, &doc.entity, cal)?;
    narrative::apply_times(&mut doc.narrative, &doc.entity, &doc.event, cal)?;
    serde_json::to_value(doc).map_err(DocumentError::raw)
}

// 可计算标签：按各实体 base_time 执行脚本，归属链 chain[0] 并入 belong_to，其余逐级生成缺失实体
fn apply_calculable_tags(
    doc: &mut WeftDoc,
    runtime: &CalculableTagRuntime,
    mode: Option<&str>,
    lang: &str,
    cal: &Calendar,
) -> Result<(), DocumentError> {
    if doc.calculable_tag.is_empty() && doc.entity.values().all(|e| e.calculable_tags.is_empty()) {
        return Ok(());
    }
    let ids: Vec<String> = doc.entity.keys().cloned().collect();
    for id in ids {
        for name in doc.entity[&id].calculable_tags.clone() {
            let entity = &doc.entity[&id];
            let base_time = entity
                .base_time
                .as_ref()
                .map(|phase| cal.normalize(&phase.flatten(cal.component_count())?))
                .transpose()?;
            let context = serde_json::json!({
                "api": 1,
                "date_mode": mode,
                "language": lang,
                "entity": {
                    "id": id,
                    "name": entity.name,
                    "base_time": base_time,
                    "description": entity.description,
                    "belong_to": entity.belong_to,
                },
            });
            let chain = runtime.calculable_tag(&name, &context, lang)?;
            let chain: Vec<String> = serde_json::from_value(chain)
                .map_err(|_| format!("可计算标签 `{name}` 必须返回字符串列表（归属链）"))?;
            apply_chain(&mut doc.entity, &id, &chain)?;
        }
    }
    // 可计算标签生成的归属可能成环（比如把实体自己算进链里），复查 belong_to 无环
    crate::weft::entity::check_acyclic(&doc.entity)
}

// 名称 → 已有实体 id：tag 返回的是名称，实体 id 可以与之不同
fn resolve_entity_id(entity: &HashMap<String, Entity>, target: &str) -> Option<String> {
    if entity.contains_key(target) {
        return Some(target.to_owned());
    }
    entity
        .iter()
        .find(|(_, e)| e.name.as_deref() == Some(target))
        .map(|(id, _)| id.clone())
}

// 归属链：实体 → chain[0] → chain[1] …。链上缺失的实体按名称自动生成，
// 撞到已有实体（id 或名称相同）即停；已有实体自己的归属不动
fn apply_chain(
    entity: &mut HashMap<String, Entity>,
    id: &str,
    chain: &[String],
) -> Result<(), String> {
    if chain.is_empty() {
        return Ok(());
    }
    // 先把每个名称解析成 id：已有实体用它自己的 id，新建实体用名称作 id。
    // 新建实体的 belong_to 因此总能指向真实存在的 id
    let existing: Vec<Option<String>> = chain
        .iter()
        .map(|target| resolve_entity_id(entity, target))
        .collect();
    let ids: Vec<String> = chain
        .iter()
        .zip(&existing)
        .map(|(target, id)| id.clone().unwrap_or_else(|| target.clone()))
        .collect();
    for (i, target) in chain.iter().enumerate() {
        if existing[i].is_some() {
            break;
        }
        entity.insert(
            target.clone(),
            Entity {
                name: Some(target.clone()),
                base_time: None,
                base_time_humanizer: None,
                description: None,
                belong_to: ids.get(i + 1).cloned().into_iter().collect(),
                calculable_tags: Vec::new(),
            },
        );
    }
    let owner = entity
        .get_mut(id)
        .ok_or_else(|| format!("可计算标签归属链引用了不存在的实体 `{id}`"))?;
    let first_id = &ids[0];
    if !owner.belong_to.contains(first_id) {
        owner.belong_to.push(first_id.clone());
    }
    Ok(())
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
