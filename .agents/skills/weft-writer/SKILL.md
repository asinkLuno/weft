---
name: weft-writer
description: >-
  Author, edit, validate, and format `.weft` timeline documents and accompanying `.rn` Rune
  plugins (custom time axes/calendars via `date_mode` and derived entity properties via
  `calculable_props`), using the `weft` CLI (`weft check`, `weft fmt`, `weft rules`) to enforce
  syntax, semantic, script, and lint correctness. Trigger this skill whenever the user wants to
  write or edit a `.weft` file, model any timeline-based subject in WEFT (historical chronologies,
  project/product roadmaps, incident postmortems, technical or organizational evolutions,
  biographies, event logs, stories/scripts, etc.), convert notes or outlines into `.weft`, write
  or debug `.rn` calendar/time-axis or calculable-property scripts for WEFT, or lint/format/fix
  diagnostics in `.weft` documents—even if they do not explicitly mention "weft check" or "weft fmt".
---

# WEFT Language Authoring & Validation

Write syntactically and semantically valid `.weft` documents (and optional `.rn` Rune plugins), then verify and format them with the `weft` CLI until both `check --deny-warnings` and `fmt --check` pass.

WEFT is a general declarative HCL-like language for **anything expressible on a timeline**—entities, hierarchical membership (DAG), typed relationships, reusable time coordinates, timestamped point/interval events, and ordered event tracks. Two top-level keywords are named `story` and `events` in the grammar; treat them strictly as structural primitives (`story` = document header & plugin registry, `events` = named ordered view over events), applicable to any domain.

## Grammar Quick Reference

A `.weft` file is a sequence of top-level declarations in any order, separated by whitespace/newlines:

| Declaration | Syntax                       | Required / Allowed Fields                                                                                                                                                       | Notes                                                                                                                                                                             |
| ----------- | ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `story`     | `story { ... }`              | **Req**: `title` (str). **Opt**: `author` (str), `description` (str), `date_mode` (str `.rn`), `calculable_props` (`{ id = "path.rn" }`)                                        | Exactly one per file; unknown keys fail (`unknown-field`).                                                                                                                        |
| `time`      | `time <name> = <time_expr>`  | —                                                                                                                                                                               | Reusable time coordinate anchor. Keep `<name>` a valid bare identifier so it can be referenced directly.                                                                          |
| `entity`    | `entity <name> { ... }`      | **Opt**: `description` (str), `base_time` (time_expr), `avatar` (str image path), `belong_to` (`[entity, ...]`), `calculable_props` (`[prop_id, ...]`), **plus any custom key** | Globally unique name. `belong_to` must be a DAG (multi-parent OK, cycles fail). Arbitrary extra keys pass into `extra_props` (scalars/lists/objects only; no time refs or `+/-`). |
| `link`      | `link <group> { ... }`       | Edges: `<entity> -- "<relation>" -- <entity>`                                                                                                                                   | Both endpoints must be declared entities. Same `<group>` across multiple blocks merges edges.                                                                                     |
| `event`     | `event <name> { ... }`       | **Req**: `start` (time_expr). **Opt**: `end` (time_expr), `entities` (`[entity, ...]`), `description` (str)                                                                     | Globally unique name. Must satisfy `end >= start`. Unknown keys fail (`unknown-field`).                                                                                           |
| `events`    | `events <name> = [ev1, ev2]` | Ordered list of declared event names                                                                                                                                            | Defines an ordered track/view. Repeating an event is allowed.                                                                                                                     |

**Time expressions** (valid in `time`, `entity.base_time`, `event.start`, `event.end`):

- **Absolute**: integer (`2026` == `[2026]`) or non-empty integer list (`[2026, 3, 15, 14, 30, 0]`). Omitted trailing components default to the calendar's `minimums`.
- **Reference**: bare time anchor (`t0`), member ref (`ev.start`, `ev.end`, `ent.base_time`, `"v2/cutover".start`), or intra-event `start` / `end`.
- **Offset**: left-associative `+` / `-` with an integer or integer list (`start + [0, 0, 0, 2]`, `t0 - [0, 1] + [0, 0, 3]`). Carries/borrows normalize automatically through the active calendar.

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
  description = "负责基础设施与发布变更的团队。"
}

entity "api/v2" {
  description = "新版业务接口服务。"
  base_time   = [2025, 11, 1]
  belong_to   = [平台组]
  kind        = "service" # entity 允许任意表外字段，原样进入 extra_props
  tier        = 1
}

entity 流量网关 {
  description = "入口七层转发网关。"
  belong_to   = [平台组]
  kind        = "infra"
}

link 拓扑依赖 {
  流量网关 -- "路由至" -- "api/v2"
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
```

## Workflow & CLI Usage

1. **Map the domain to the 6 primitives**: decide what serves as `entity` (nodes, actors, systems, places, milestones, groups via `belong_to` DAG), `link` (typed relations), `time` (shared coordinate anchors), `event` (timestamped points/intervals), and `events` (ordered views/tracks).
2. **Author `.weft` (and `.rn` plugins if needed)**:
   - When the timeline needs a non-Gregorian coordinate system (fiscal quarters, sprint cycles, elapsed mission/experiment time, historical or custom calendars) or derived entity fields, read [`references/rune-plugins.md`](references/rune-plugins.md) before writing `.rn` scripts so exported signatures, `ctx` shapes, and round-trip invariants match the Rust host.
   - For complete lexical rules, edge cases, time math, domain mappings, formatting rules, or diagnostic codes, read [`references/syntax-reference.md`](references/syntax-reference.md).
3. **Verify & format in a tight loop** via the bundled wrapper `scripts/run_weft.sh` (relative to this skill directory), which resolves `weft` automatically from `PATH`, installed desktop app, `<repo>/target/{release,debug}/weft`, or `cargo run -p weft-cli`:

```bash
WEFT="<path-to-this-skill>/scripts/run_weft.sh"

# 1. Fast draft check (skips .rn execution and avatar existence check)
"$WEFT" check path/to/doc.weft --syntax-only --output-format concise

# 2. Full check: syntax + semantics + .rn execution + lint (warnings fail)
"$WEFT" check path/to/doc.weft --deny-warnings

# 3. Auto-format in place (refuses syntax-broken files; preserves comments)
"$WEFT" fmt path/to/doc.weft

# 4. Final verification gate before replying
"$WEFT" check path/to/doc.weft --deny-warnings && "$WEFT" fmt --check path/to/doc.weft
```

Always run the full check (without `--syntax-only`) before finishing—`--syntax-only` skips `.rn` compilation/execution, calendar round-trip verification, and avatar path checks. Fix all errors and warnings rather than hiding them with `--ignore` unless the user explicitly asks to keep unused draft stubs. Use `--lang zh` and `--lang en` to test bilingual `.rn` scripts; run `"$WEFT" rules` to print active lint rules.

## High-Impact Syntax & Semantic Pitfalls

- **Quote typed string fields**: `title`, `author`, `description`, `avatar`, and `date_mode` strictly require `"..."` or `"""..."""`. Bare identifiers fail with `invalid-field`.
- **Identifier quoting vs time-anchor refs**: Bare names start with a Unicode letter or `_` and continue with letters, digits, `_`, `·`, `・` (cannot be `true`/`false`/`null`). Names with spaces, `/`, `-`, punctuation, or leading digits must be double-quoted (`entity "api/v2"`, `"api/v2".base_time`). **Exception for `time` anchors**: anchor references only parse bare identifiers, so always give `time` anchors valid bare names (not quoted names).
- **List vs block separators**: Lists (`[...]`) and `events` require commas `,` between items (trailing comma allowed). Attributes inside `{ ... }` separate by newlines or optional `,`/`;`.
- **Extra fields only on `entity`**: Arbitrary keys are allowed on `entity` (merged into `extra_props`, cannot contain time refs or `+`/`-`), whereas `story` and `event` reject any unknown key (`unknown-field`).
- **Calendar bounds & negative roots**: Built-in Gregorian uses 6 components `[year, month, day, hour, minute, second]` with `minimums = [0, 1, 1, 0, 0, 0]`. The first component has no lower bound (negative values like `[-44, 3, 15]` are valid), while components `i > 0` must be `>= minimums[i]` (`[2026, 0, 1]` fails with `time-component-below-minimum`). Keep `end >= start` and avoid time cycles.
- **Keep all 7 lint rules clean**:
  - `unused-entity`: When the file has any `event` or `link`, every `entity` must appear in at least one `event.entities`, `link`, or another entity's `belong_to`.
  - `unreferenced-event`: When any `events` declaration exists, include every `event` in at least one `events` list.
  - `unused-time-anchor`: Only declare `time` anchors that are actually referenced.
  - `self-link`, `duplicate-link`, `duplicate-attr`, `empty-block`: Avoid self-edges, duplicate edges in a `link` group, repeated keys in a block, and empty `link` or `events` declarations.

## Reference Guides

- **Full grammar, time semantics, domain mapping, formatting & diagnostics**: [`references/syntax-reference.md`](references/syntax-reference.md)
- **Custom time axes/calendars (`date_mode`) & derived entity properties (`calculable_props`) in Rune (`.rn`)**: [`references/rune-plugins.md`](references/rune-plugins.md)
