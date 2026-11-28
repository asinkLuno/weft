---
name: weft-writer
description: >-
  Author, edit, validate, and format `.weft` timeline documents and accompanying `.rn` Rune
  plugins (custom time axes/calendars via `date_mode` and derived entity properties via
  `calculable_props`), using the `weft` CLI (`weft check`, `weft fmt`, `weft rules`) to enforce
  syntax, semantic, script, and lint correctness. Trigger this skill whenever the user wants to
  write or edit a `.weft` file, model any timeline-based subject in WEFT (stories/novels with plot
  foreshadowing & blocking, historical chronologies, project/product roadmaps, incident postmortems,
  technical or organizational evolutions, biographies, event logs, etc.), convert notes or outlines
  into `.weft`, write or debug `.rn` calendar/time-axis or calculable-property scripts for WEFT, or
  lint/format/fix diagnostics in `.weft` documents—even if they do not explicitly mention "weft check" or "weft fmt".
---

# WEFT Language Authoring & Validation

Write syntactically and semantically valid `.weft` documents (and optional `.rn` Rune plugins), then verify and format them with the `weft` CLI until both `check --deny-warnings` and `fmt --check` pass.

WEFT is a general declarative HCL-like language for **novels and anything expressible on a timeline**—metadata, time anchors, entities, timestamped events (plot points), ordered event tracks, and homogeneous `link` graphs (either temporal `entity` state graphs or `event` plot/dependency DAGs, with algebraic constraints orthogonal to edge labels). Two top-level keywords are named `story` and `events`; treat them strictly as structural primitives (`story` = document header & plugin registry, `events` = named ordered view over events), applicable to any domain.

## Grammar Quick Reference

A `.weft` file is a sequence of **6 orthogonal top-level declarations** in any order (forward references are allowed):

| Declaration | Syntax                       | Required / Allowed Fields                                                                                                                                                                                                                                                                                                                                                                                              | Notes                                                                                                                                                                                                                            |
| ----------- | ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `story`     | `story { ... }`              | **Req**: `title` (str). **Opt**: `author` (str), `description` (str), `date_mode` (str `.rn`), `calculable_props` (`{ id = "path.rn" }`)                                                                                                                                                                                                                                                                               | Exactly one per file; unknown keys fail (`unknown-field`).                                                                                                                                                                       |
| `time`      | `time <name> = <time_expr>`  | —                                                                                                                                                                                                                                                                                                                                                                                                                      | Reusable time coordinate anchor. Always declare `<name>` as a bare identifier (quoted anchor names cannot be referenced in time expressions).                                                                                    |
| `entity`    | `entity <name> { ... }`      | **Opt**: `description` (str), `base_time` (time_expr), `avatar` (str image path), `calculable_props` (`[prop_id, ...]`), **plus any custom key**                                                                                                                                                                                                                                                                       | Shares unified symbol namespace with `event` (`duplicate-symbol`). No built-in `belong_to`/`tags`; use `link` graphs for structure and custom keys (e.g. `kind = "..."`) for `extra_props` (no `.`/`+`/`-`).                     |
| `event`     | `event <name> { ... }`       | **Req**: `start` (time_expr). **Opt**: `end` (time_expr), `entities` (`[entity, ...]`), `description` (str)                                                                                                                                                                                                                                                                                                            | Shares unified symbol namespace with `entity` (`duplicate-symbol`). Must satisfy `end >= start`. Sole place to bind participating entities to an event. Unknown keys fail (`unknown-field`).                                     |
| `events`    | `events <name> = [ev1, ev2]` | Ordered list of declared event names                                                                                                                                                                                                                                                                                                                                                                                   | Defines an ordered narrative track/view without altering event timestamps. Repeating an event is allowed.                                                                                                                        |
| `link`      | `link <graph> { ... }`       | **Opt graph attrs** (booleans default `false`): `acyclic`, `asymmetric`, `intransitive`, `functional`, `inverse_functional`, `symmetric`, `transitive`, `equivalence`, `chronological`; `description` (str). **Edges**: `A -> B`, `A -- "label" -> B`, `A <-> B`, `A <- "label" -> B`, cliques `<-> [A, B, C]` / `<- "label" -> [A, B, C]`, batch lists `[A, B] -> C`; optional trailing `{ start, end, description }` | Uniquely named homogeneous graph (`duplicate-link-graph`, `mixed-link-node-kinds`): either all `entity` nodes (temporal `start`/`end` allowed except on `equivalence`) or all `event` nodes (no `start`/`end` or `equivalence`). |

**Common `link` modeling patterns**:

| Pattern                       | Node Domain | Graph Attributes                         | Edge Syntax / Notes                                                              |
| ----------------------------- | ----------- | ---------------------------------------- | -------------------------------------------------------------------------------- |
| Membership / org hierarchy    | `entity`    | `acyclic = true`, `transitive = true`    | Directed `child -> parent`, optional `{ start, end }`                            |
| Spatial containment / tree    | `entity`    | `acyclic = true`, `functional = true`    | Directed `child -> container`; out-degree `<= 1` per time slice                  |
| Unique item / role ownership  | `entity`    | `inverse_functional = true`              | Directed `owner -> item`; sequential hand-offs with non-overlapping intervals OK |
| Strict direct hierarchy       | `entity`    | `acyclic = true`, `intransitive = true`  | Forbids shortcut `A -> C` when `A -> B -> C` exists in the same slice            |
| Identity alias / quotient     | `entity`    | `equivalence = true`                     | Undirected `<->` / clique only; timeless (forbids `start`/`end`)                 |
| Pure symmetric relation       | `entity`    | `symmetric = true`                       | Undirected `<->` / `<- "label" ->` / clique only                                 |
| Free relationship / call mesh | `entity`    | *(none)*                                 | Mix directed, undirected, and cliques freely; distinguish semantics via labels   |
| Plot blocking / foreshadowing | `event`     | `acyclic = true`, `chronological = true` | Directed `prereq -- "reason" -> target`; enforces `prereq.start <= target.start` |

**Time expressions** (valid in `time`, `entity.base_time`, `event.start`, `event.end`, and non-equivalence `entity` `link` edge `start`/`end`):

- **Absolute**: integer (`2026` == `[2026]`) or non-empty integer list (`[2026, 3, 15, 14, 30, 0]`). Component `0` has no lower bound (negative allowed); components `i > 0` must be `>= minimums[i]`. Omitted trailing components fill from `minimums` while preserving display precision.
- **Reference**: bare time anchor (`t0`), member ref (`ev.start`, `ev.end`—falls back to `ev.start` on point events, `ent.base_time`, `"v2/cutover".start`), or intra-block `start` / `end` inside an `event` or `link` edge block.
- **Offset**: left-associative `+` / `-` with an integer or non-empty integer list (`start + [0, 0, 0, 2]`, `t0 - [0, 1] + [0, 0, 3]`). Carries/borrows normalize automatically through the active calendar.

## Minimal `.weft` Skeleton

```weft
story {
  title       = "核心服务迁移时间轴"
  author      = "平台工程组"
  description = """
    记录网关与 API 服务分阶段割接的关键节点。
    多行字符串会自动去除公共缩进。
  """
  # date_mode        = "sprint_axis.rn"
  # calculable_props = { stage = "stage.rn" }
}

time 割接窗口 = [2026, 3, 15, 2]

entity 平台组 {
  kind        = "team"
  description = "负责基础设施与发布变更的团队。"
}

entity "api/v2" {
  description = "新版业务接口服务。"
  base_time   = [2025, 11, 1]
  kind        = "service" # entity 允许任意表外字段，原样进入 extra_props
  tier        = 1
}

entity 流量网关 {
  description = "入口七层转发网关。"
  kind        = "infra"
}

event 灰度放量 {
  start       = 割接窗口
  end         = start + [0, 0, 0, 1, 30]
  entities    = [流量网关, "api/v2"]
  description = "将 10% 生产流量切至新版接口并观察指标。"
}

event 全量切换 {
  start       = 灰度放量.end + [0, 0, 0, 0, 30]
  entities    = [流量网关, "api/v2", 平台组]
  description = "确认无异常后完成全量切流。"
}

events 发布主线 = [灰度放量, 全量切换]

link 团队归属 {
  acyclic    = true
  transitive = true

  ["api/v2", 流量网关] -> 平台组
}

link 服务调用 {
  流量网关 -- "路由至" -> "api/v2" {
    start = 灰度放量.start
  }
}

link 变更依赖 {
  acyclic       = true
  chronological = true

  灰度放量 -- "指标平稳" -> 全量切换
}
```

## Workflow & CLI Usage

1. **Map the domain to the 6 primitives**: separate `entity` state graphs (membership DAG, spatial tree, unique ownership, equivalence, or unconstrained temporal `link` graphs) from `event` plot/dependency graphs (`acyclic + chronological` for foreshadowing, blocking prerequisites, and causal chains), then model `entity`, `time`, `event`, and `events`.
2. **Author `.weft` (and `.rn` plugins if needed)**:
   - When the timeline needs a non-Gregorian coordinate system or derived entity fields, read [`references/rune-plugins.md`](references/rune-plugins.md).
   - For complete lexical rules, algebraic graph constraints, time resolution semantics, domain mapping, formatting rules, or diagnostic fixes, read [`references/syntax-reference.md`](references/syntax-reference.md).
3. **Verify & format in a tight loop** via `scripts/run_weft.sh`:

```bash
WEFT="<path-to-this-skill>/scripts/run_weft.sh"

# 1. Fast draft check (skips .rn execution, avatar check, and timed slice resolution; still checks timeless edges)
"$WEFT" check path/to/doc.weft --syntax-only --output-format concise

# 2. Full check: syntax + semantics + atomic time-slice / chronological graph checks + .rn + lint
"$WEFT" check path/to/doc.weft --deny-warnings

# 3. Auto-format in place (refuses syntax-broken files; preserves comments)
"$WEFT" fmt path/to/doc.weft

# 4. Final verification gate before replying
"$WEFT" check path/to/doc.weft --deny-warnings && "$WEFT" fmt --check path/to/doc.weft

# Optional: compile to structured WeftDoc JSON
"$WEFT" build path/to/doc.weft -o dist/doc.json
```

## High-Impact Syntax & Semantic Pitfalls

- **Quote typed string fields & edge labels**: `title`, `author`, `description`, `avatar`, `date_mode`, and edge labels (`-- "label" ->` / `<- "label" ->`) strictly require `"..."` (or `"""..."""` for descriptions). Names with spaces, slashes, leading digits, punctuation other than `·`/`・`, or `true`/`false`/`null` must also be double-quoted.
- **Keep `time` anchor names bare**: `time` anchors referenced in time expressions only match bare identifiers (`Expr::Ident`); never quote a `time` anchor name.
- **Unified symbol namespace + homogeneous `link` graphs**: `entity` and `event` cannot share names (`duplicate-symbol`). Never mix `entity` and `event` endpoints in the same `link` block (`mixed-link-node-kinds`). `event` graphs and `equivalence` graphs forbid `start`/`end` on edges (`event-link-time-bounds`, `equivalence-link-time-bounds`); `event` graphs also forbid `equivalence` (`event-graph-equivalence`).
- **Direction & mutual-exclusion constraints**: `acyclic`/`asymmetric`/`chronological` require directed edges (`->` / `-- "..." ->`) and conflict with `symmetric`/`equivalence`; `symmetric`/`equivalence` require `<->` / `<- "..." ->` / cliques; `transitive` conflicts with `intransitive`; `equivalence` conflicts with `intransitive`/`functional`/`inverse_functional` (`invalid-graph-config`, `arrow-direction-mismatch`).
- **Atomic time-slice intervals**: `entity` edge intervals are half-open `[start, end)` (or instant `[t, t]` when `start == end`), defaulting to `[-∞, +∞)`. Sequential edges meeting at a boundary (`end = T`, `start = T`) do **not** overlap.
- **Keep all 7 lint rules clean**: `duplicate-attr`, `unused-time-anchor`, `unused-entity`, `unreferenced-event`, `self-link`, `duplicate-link`, `empty-block`.

## Reference Guides

- **Full grammar, algebraic graphs, time semantics, domain mapping, diagnostics & formatting**: [`references/syntax-reference.md`](references/syntax-reference.md)
- **Custom time axes/calendars (`date_mode`) & derived entity properties (`calculable_props`) in Rune (`.rn`)**: [`references/rune-plugins.md`](references/rune-plugins.md)
