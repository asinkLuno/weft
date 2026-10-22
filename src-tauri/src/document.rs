use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use notify::RecursiveMode;
use notify_debouncer_full::{Debouncer, RecommendedCache, new_debouncer};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_log::log::{error, warn};

use crate::calendar::Calendar;
use crate::model::StoryDoc;
use crate::validation::{DocumentError, ValidationError};

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

// base 是故事文件所在目录，自定义历法的脚本路径相对它
// 事件引用按 id 对回实体，取其 base_time
fn resolve_base<'a>(
    entity: &'a std::collections::HashMap<String, crate::model::Entity>,
    r: &str,
) -> Option<&'a Vec<i64>> {
    entity.get(r).and_then(|e| e.base_time.as_ref())
}

fn offset_display(cal: &Calendar, time: &[i64], base: &[i64]) -> Result<String, DocumentError> {
    cal.offset_display(time, base)
}

fn parse(content: &str, base: &Path, lang: &str) -> Result<Value, DocumentError> {
    let yaml: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(content).map_err(DocumentError::raw)?;
    // 序列化 model.rs 声明过的结构：未知键在这里被滤掉
    let mut doc = StoryDoc::from_yaml(&yaml)?;
    // 加载历法并解析所有时间：历法错误等同 YAML 错误，阻止展示
    let mode = doc
        .story
        .date_mode
        .clone()
        .unwrap_or_else(|| "gregorian".into());
    let spec = doc.calendar.get(&mode);
    let cal = Calendar::load(&mode, spec, base, lang)?;
    doc.calendar_meta = Some(cal.metadata());
    for entity in doc.entity.values_mut() {
        if let Some(values) = &entity.base_time {
            let normalized = cal.normalize(values)?;
            entity.base_time_display = Some(cal.humanize(&normalized)?);
            entity.base_time = Some(normalized);
        }
    }
    apply_calculable_tags(&mut doc, base, &mode, lang)?;
    // 事件：展平相对时间 → 归一化 → tick（排序与先后校验的基准）→ humanize
    for events in doc.event.values_mut() {
        for ev in &mut *events {
            let start = cal.normalize(&ev.start_phase.flatten())?;
            let start_tick = cal.to_tick(&start)?;
            let end = ev
                .end_phase
                .as_ref()
                .map(|p| cal.normalize(&p.flatten()))
                .transpose()?;
            let end_tick = end.as_ref().map(|v| cal.to_tick(v)).transpose()?;
            if end_tick.is_some_and(|end| end < start_tick) {
                return Err(ValidationError::EventEndBeforeStart {
                    event: ev.id.clone(),
                }
                .into());
            }
            let start_display = cal.humanize(&start)?;
            let end_display = match &end {
                Some(v) => Some(cal.humanize(v)?),
                None => None,
            };
            // 参与者相对各自 base_time 的偏移；没有 base_time 的实体 start 留空
            let mut offsets = Vec::with_capacity(ev.entities.len());
            for r in &ev.entities {
                let off = resolve_base(&doc.entity, r);
                let start_off = off.map(|b| offset_display(&cal, &start, b)).transpose()?;
                let end_off = match (&off, &end) {
                    (Some(b), Some(t)) => offset_display(&cal, t, b).map(Some)?,
                    _ => None,
                };
                offsets.push(crate::model::EntityOffset {
                    entity: r.clone(),
                    start: start_off,
                    end: end_off,
                });
            }
            ev.offsets = offsets;
            ev.start = Some(start);
            ev.end = end;
            ev.start_tick = Some(start_tick);
            ev.end_tick = end_tick;
            ev.start_display = Some(start_display);
            ev.end_display = end_display;
        }
        events.sort_by_key(|e| e.start_tick.unwrap_or(i64::MAX));
    }
    // 叙事线：subject 事件相对 observer base_time 的偏移（tick 定位 + humanize 展示）
    let StoryDoc {
        entity,
        event,
        narrative,
        ..
    } = &mut doc;
    for nar in narrative.values_mut() {
        let base = resolve_base(entity, &nar.observer);
        let base_tick = base.map(|b| cal.to_tick(b)).transpose()?;
        for s in &mut nar.subject {
            let Some((g, t)) = s.event.split_once('/') else {
                continue;
            };
            let Some(ev) = event
                .get(g)
                .and_then(|evs| evs.iter().find(|e| e.title == t))
            else {
                continue;
            };
            s.start_tick = base_tick
                .zip(ev.start_tick)
                .map(|(b, t)| {
                    t.checked_sub(b)
                        .ok_or(ValidationError::TimeOffsetOutOfRange)
                })
                .transpose()?;
            s.end_tick = base_tick
                .zip(ev.end_tick)
                .map(|(b, t)| {
                    t.checked_sub(b)
                        .ok_or(ValidationError::TimeOffsetOutOfRange)
                })
                .transpose()?;
            s.start = match (base, &ev.start) {
                (Some(b), Some(t)) => Some(offset_display(&cal, t, b)?),
                _ => None,
            };
            s.end = match (base, &ev.end) {
                (Some(b), Some(t)) => Some(offset_display(&cal, t, b)?),
                _ => None,
            };
        }
        nar.subject
            .sort_by_key(|s| s.start_tick.unwrap_or(i64::MAX));
    }
    serde_json::to_value(doc).map_err(DocumentError::raw)
}

// 可计算标签：按各实体 base_time 执行脚本，归属链 chain[0] 并入 belong_to，其余逐级生成缺失实体
fn apply_calculable_tags(
    doc: &mut StoryDoc,
    base: &Path,
    mode: &str,
    lang: &str,
) -> Result<(), DocumentError> {
    if doc.calculable_tag.is_empty() && doc.entity.values().all(|e| e.calculable_tags.is_empty()) {
        return Ok(());
    }
    let runtime = crate::calculable_tag::CalculableTagRuntime::load(&doc.calculable_tag, base)?;
    let ids: Vec<String> = doc.entity.keys().cloned().collect();
    for id in ids {
        for name in doc.entity[&id].calculable_tags.clone() {
            let entity = &doc.entity[&id];
            let context = serde_json::json!({
                "api": 1,
                "date_mode": mode,
                "language": lang,
                "entity": {
                    "id": id,
                    "name": entity.name,
                    "base_time": entity.base_time,
                    "description": entity.description,
                    "belong_to": entity.belong_to,
                },
            });
            let chain = runtime.calculable_tag(&name, &context)?;
            let chain: Vec<String> = serde_json::from_value(chain)
                .map_err(|_| format!("可计算标签 `{name}` 必须返回字符串列表（归属链）"))?;
            apply_chain(&mut doc.entity, &id, &chain)?;
        }
    }
    // 可计算标签生成的归属可能成环（比如把实体自己算进链里），复查 belong_to 无环
    crate::model::check_acyclic(&doc.entity)
}

// 归属链：实体 → chain[0] → chain[1] …；缺失的环上实体自动生成，撞到已有实体即停（YAML 自定义的归属不动）
fn apply_chain(
    entity: &mut std::collections::HashMap<String, crate::model::Entity>,
    id: &str,
    chain: &[String],
) -> Result<(), String> {
    let Some(first) = chain.first() else {
        return Ok(());
    };
    for (i, target) in chain.iter().enumerate() {
        let exists = entity.contains_key(target)
            || entity.values().any(|e| e.name.as_deref() == Some(target));
        if exists {
            break;
        }
        entity.insert(
            target.clone(),
            crate::model::Entity {
                name: Some(target.clone()),
                base_time: None,
                base_time_display: None,
                description: None,
                belong_to: chain.get(i + 1).cloned().into_iter().collect(),
                calculable_tags: Vec::new(),
            },
        );
    }
    let owner = entity
        .get_mut(id)
        .ok_or_else(|| format!("可计算标签归属链引用了不存在的实体 `{id}`"))?;
    if !owner.belong_to.contains(first) {
        owner.belong_to.push(first.clone());
    }
    Ok(())
}

fn read_doc(
    path: &Path,
    lang: &str,
) -> Result<(String, Result<Value, DocumentError>), DocumentError> {
    let content = fs::read_to_string(path).map_err(DocumentError::raw)?;
    // 语法错误不算绑定失败：正常进入展示页，错误原样带给前端
    let parsed = parse(&content, path.parent().unwrap_or(Path::new(".")), lang);
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
        &content,
        path.parent().unwrap_or(Path::new(".")),
        &crate::current_lang(app),
    ) {
        Ok(doc) => DocumentPayload {
            doc: Some(doc),
            content: Some(content),
            error: None,
        },
        Err(error) => {
            error!(
                "failed to parse changed document {}: {error}",
                path.display()
            );
            DocumentPayload {
                doc: None,
                content: None,
                error: Some(error),
            }
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
    let payload = match read_doc(&path, &crate::current_lang(app)) {
        Ok((content, Ok(doc))) => DocumentPayload {
            doc: Some(doc),
            content: Some(content),
            error: None,
        },
        Ok((_, Err(error))) | Err(error) => DocumentPayload {
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
) -> Result<DocumentPayload, DocumentError> {
    let path = PathBuf::from(path);
    let (content, parsed) = read_doc(&path, &crate::current_lang(&app)).inspect_err(|error| {
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
        Ok(doc) => DocumentPayload {
            doc: Some(doc),
            content: Some(content),
            error: None,
        },
        Err(error) => {
            error!("failed to parse document {}: {error}", path.display());
            DocumentPayload {
                doc: None,
                content: Some(content),
                error: Some(error),
            }
        }
    })
}

#[tauri::command]
pub fn save_document(
    app: AppHandle,
    state: State<'_, Mutex<DocumentState>>,
    content: String,
) -> Result<(), DocumentError> {
    // 非法 YAML / 历法错误不落盘；目录取自当前绑定，保存不改路径
    let st_guard = state.lock().unwrap_or_else(|p| p.into_inner());
    let base = st_guard
        .path
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_default();
    drop(st_guard);
    parse(&content, &base, &crate::current_lang(&app)).inspect_err(|error| {
        error!("failed to validate document before saving: {error}");
    })?;
    let mut st = state.lock().unwrap_or_else(|p| p.into_inner());
    let path = st
        .path
        .clone()
        .ok_or(DocumentError::validation(ValidationError::DocumentNotBound))?;
    st.last_written = Some(content.clone());
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
        st.last_written = None;
        let _ = fs::remove_file(&tmp);
        return Err(DocumentError::raw(e));
    }
    Ok(())
}

#[tauri::command]
pub fn unbind_document(state: State<'_, Mutex<DocumentState>>) {
    *state.lock().unwrap_or_else(|p| p.into_inner()) = DocumentState::default();
}
