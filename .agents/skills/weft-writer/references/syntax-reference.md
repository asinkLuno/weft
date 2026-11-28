# WEFT Syntax, Semantics & Diagnostics Reference

Complete language reference for authoring, debugging, and formatting `.weft` files with `weft check`, `weft fmt`, and `weft build`.

## Contents

1. [Lexical Rules](#lexical-rules)
2. [Top-Level Declarations](#top-level-declarations)
3. [Time Expressions & Resolution](#time-expressions--resolution)
4. [Mapping Timeline Domains to WEFT](#mapping-timeline-domains-to-weft)
5. [Diagnostics & Lint Reference](#diagnostics--lint-reference)
6. [Formatting Specification (`weft fmt`)](#formatting-specification-weft-fmt)

______________________________________________________________________

## Lexical Rules

- **Comments**:
  - Line comments: `# ...` and `// ...` until end of line.
  - Block comments: `/* ... */` across lines.
  - Comment markers inside string literals are literal text. `weft fmt` preserves comments.
- **Names (identifiers)**:
  - **Bare name**: starts with a Unicode alphabetic character or `_`; continues with Unicode alphanumeric characters, `_`, `·`, or `・`. Cannot be `true`, `false`, or `null`.
    - Valid bare: `平台组`, `node_1`, `哈利·波特`, `T0`
    - Must be double-quoted: `"api/v2"`, `"2026-Q1"`, `"Phase 1"`, `"502-网关"`, `"9又3/4站台"`, `"true"`
  - Anywhere a `<name>` is expected (`entity`, `link` graph name/endpoints, `event`, `events`, attribute keys, member access base), either a bare name or a double-quoted string is parsed—**except** referencing a `time` anchor inside a time expression, which only resolves bare identifiers (`Expr::Ident`). Always declare `time` anchors with bare names.
- **Strings**:
  - Single-line `"..."` supports escapes `\\`, `\"`, `\n`, `\r`, `\t`.
  - Multi-line `"""..."""` strips the opening/closing boundary newlines and removes common leading indentation across non-empty lines. Ideal for multi-line `description` blocks.
- **Numbers & literals**:
  - Signed integers (`42`, `-44`), signed floats (`3.14`, `-0.5`), booleans (`true`, `false`), `null`.
  - Optional string fields (`author`, `description`, `date_mode`, `avatar`) and optional time fields (`entity.base_time`, `event.end`, `link` edge `start`/`end`) also accept `null` (equivalent to omitting the field).
- **Operators**:
  - Link arrows: `->`, `<->`, `--`, `<-`.
  - Time offset operators: `+`, `-`.
- **Collections & separators**:
  - **Lists** `[e1, e2, ...]`, **`events` lists**, **link batch endpoint lists**, and **clique member lists**: elements **must** be separated by `,` (trailing comma allowed).
  - **Attribute blocks / objects** `{ k = v ... }` and **`link` block bodies**: items are separated by newlines, spaces, or optional `,` / `;`. Keys can be bare or quoted names.
- **Bare identifier vs quoted string semantics**:
  - Typed string fields (`story.title`, `story.author`, `story.description`, `story.date_mode`, `entity.description`, `entity.avatar`, `event.description`, `link.description`, edge `description`) strictly require string literals (`"..."` / `"""..."""`); bare identifiers fail with `invalid-field`.
  - Edge labels (`-- "label" ->`, `<- "label" ->`) strictly require double-quoted strings; bare identifiers fail with `syntax-error`.
  - Name lists (`entities`, `calculable_props`, `events`, `link` endpoints and clique members) accept both bare and quoted names and resolve symbol references.
  - In `entity` custom extra fields (`extra_props`), a bare identifier evaluates to a JSON string (explicit quotes are clearer), while `.` member accesses and `+`/`-` expressions fail with `invalid-field` (expressions are only supported in time fields).

______________________________________________________________________

## Top-Level Declarations

WEFT has **6 orthogonal top-level declarations**. Top-level items may appear in any order; forward references across all blocks are resolved during compilation.

- **Unified node namespace & homogeneous two-layer graph architecture**:
  - `entity` and `event` share a single symbol namespace; declaring an `entity` and an `event` with the same name fails with `duplicate-symbol`.
  - `entity` has no built-in hierarchy or tag fields (`belong_to` and `tags` do not exist); all structured relations live in `link` graphs, while entity metadata lives in `extra_props`.
  - Cross-layer participation (which entities appear in an event) is declared **solely** via `event.entities`.
  - Each `link` block is strictly homogeneous (`mixed-link-node-kinds`): either exclusively `entity` nodes (temporal world-state graph) or exclusively `event` nodes (plot/dependency topology graph).

### `story { ... }` (required, exactly one)

Document header and Rune plugin registry. Missing fails with `missing-story`; duplicate fails with `duplicate-story`; unknown keys fail with `unknown-field`.

| Field              | Required | Type                        | Notes                                                                              |
| ------------------ | -------- | --------------------------- | ---------------------------------------------------------------------------------- |
| `title`            | Yes      | String                      | Document / timeline title                                                          |
| `author`           | No       | String                      | Author or owner                                                                    |
| `description`      | No       | String                      | Overview / summary                                                                 |
| `date_mode`        | No       | String                      | Relative path to `.rn` calendar / time-axis script; defaults to built-in Gregorian |
| `calculable_props` | No       | Object `{ id = "path.rn" }` | Maps prop IDs to relative `.rn` script paths                                       |

### `time <name> = <time_expr>`

Declares a reusable named coordinate anchor usable in `entity.base_time`, `event.start`, `event.end`, non-equivalence `entity` `link` edge `start`/`end`, or another `time` anchor. Duplicate names fail with `duplicate-time-anchor`. Use bare names so they can be referenced in time expressions.

### `entity <name> { ... }`

Declares a participant, character, service, organization, place, artifact, or concept. Duplicate names fail with `duplicate-entity` (or `duplicate-symbol` if colliding with an `event`).

| Field              | Type                   | Notes                                                                                                     |
| ------------------ | ---------------------- | --------------------------------------------------------------------------------------------------------- |
| `description`      | String                 | Entity summary                                                                                            |
| `base_time`        | Time expr              | Origin, birth, creation, or launch coordinate; usable in time refs, calculable props, and `entity_deltas` |
| `avatar`           | String                 | Relative path to an existing `.bmp`/`.gif`/`.ico`/`.jpeg`/`.jpg`/`.png`/`.svg`/`.webp` file               |
| `calculable_props` | List of prop IDs       | IDs registered in `story.calculable_props`; outputs merge into `extra_props`                              |
| *any other key*    | Scalar / list / object | Stored verbatim in `extra_props` (e.g. `kind = "service"`, `tier = 1`); never triggers `unknown-field`    |

### `event <name> { ... }`

Declares a timestamped event or plot point. Duplicate names fail with `duplicate-event` (or `duplicate-symbol` if colliding with an `entity`). Unknown keys fail with `unknown-field`.

| Field         | Required | Type                 | Notes                                                                                                   |
| ------------- | -------- | -------------------- | ------------------------------------------------------------------------------------------------------- |
| `start`       | Yes      | Time expr            | Start coordinate                                                                                        |
| `end`         | No       | Time expr            | End coordinate; omit for an instant point event. Must satisfy `end >= start` (`event-end-before-start`) |
| `entities`    | No       | List of entity names | Declared `entity` names participating in this event (`unknown-event-entity` if undeclared)              |
| `description` | No       | String               | Event details                                                                                           |

For every participating entity in `entities` that defines `base_time`, the compiler automatically computes calendar-formatted relative deltas (`entity_deltas`, e.g. character age or service uptime at the event) using `delta_humanizer` (or signed tick delta if omitted).

### `events <name> = [event1, event2, ...]`

Declares an ordered narrative track / curated sequence of declared events. Track order controls presentation order without altering event timestamps. Repeating an event in a list is valid. Duplicate track names fail with `duplicate-events`; undeclared events fail with `unknown-events-event`.

### `link <graph> { ... }`

Each `link` block declares a **uniquely named homogeneous graph** (`duplicate-link-graph` on duplicate graph names, `mixed-link-node-kinds` if mixing `entity` and `event` nodes). Graph-level algebraic structure constraints are completely orthogonal to edge-level semantic labels. Graph attributes and edge statements may appear in any order inside the block (`weft fmt` normalizes attributes to the top, followed by a blank line and edges).

#### Optional block-level graph attributes

All booleans default to `false`; unknown keys fail with `unknown-field`.

| Field                | Type    | Semantics & Enforcement                                                                                                                                                                         |
| -------------------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `acyclic`            | Boolean | DAG across every atomic time slice (`graph-cycle`); requires directed edges; implies irreflexive & asymmetric                                                                                   |
| `asymmetric`         | Boolean | Forbids bidirectional edges and self-loops in the same slice (`asymmetry-conflict`, cycles of length `>= 3` allowed); requires directed edges                                                   |
| `intransitive`       | Boolean | Forbids shortcut `A -> C` when `A -> B` and `B -> C` (distinct `A, B, C`) exist in the same slice (`intransitivity-violation`)                                                                  |
| `functional`         | Boolean | Out-degree `<= 1` (distinct targets) per source node per slice (`functional-conflict`); combines with `acyclic` for strict trees                                                                |
| `inverse_functional` | Boolean | In-degree `<= 1` (distinct sources) per target node per slice (`inverse-functional-conflict`); e.g. unique item ownership                                                                       |
| `symmetric`          | Boolean | Undirected graph; requires `<->`, `<- "..." ->`, or cliques; duplicate-link checks ignore endpoint order                                                                                        |
| `transitive`         | Boolean | Marks graph as transitive for closure computation in UI and queries                                                                                                                             |
| `equivalence`        | Boolean | Entity-only equivalence graph (`event-graph-equivalence` on events); requires `<->` / cliques; forbids edge `start`/`end`; enables quotient-node merging                                        |
| `chronological`      | Boolean | Directed edges only; enforces `source <= target` time (`chronological-violation`): compares `start` on `event` graphs, or `base_time` on `entity` graphs when both endpoints define `base_time` |
| `description`        | String  | Documentation shown in LSP Hover and UI                                                                                                                                                         |

**Mutually exclusive pairs** fail with `invalid-graph-config`:

- `acyclic`, `asymmetric`, or `chronological` with `symmetric` or `equivalence`
- `transitive` with `intransitive`
- `equivalence` with `intransitive`, `functional`, or `inverse_functional`

#### Edge & clique statements

| Form              | Unlabeled       | Labeled                   | Notes                                                                                                      |
| ----------------- | --------------- | ------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Directed edge     | `A -> B`        | `A -- "label" -> B`       | Directed from `A` to `B`                                                                                   |
| Symmetric edge    | `A <-> B`       | `A <- "label" -> B`       | Undirected / symmetric between `A` and `B`                                                                 |
| Batch endpoints   | `[A, B] -> C`   | `[A, B] -- "label" -> C`  | Either or both sides may be a non-empty list; expands to Cartesian product (`empty-link-endpoint` on `[]`) |
| Clique (complete) | `<-> [A, B, C]` | `<- "label" -> [A, B, C]` | Requires `>= 2` nodes (`invalid-clique`); expands to all unordered pairs sharing trailing attrs            |

- **Direction validation** (`arrow-direction-mismatch`):
  - Graphs with `acyclic`, `asymmetric`, or `chronological` allow **only directed edges** (`->` / `-- "..." ->`).
  - Graphs with `symmetric` or `equivalence` allow **only undirected edges and cliques** (`<->` / `<- "..." ->`).
  - Unconstrained (free) graphs allow mixing directed edges, symmetric edges, and cliques freely.

#### Edge attribute blocks & atomic time-slice validation

Any edge or clique statement may end with an optional `{ ... }` attribute block allowing `start`, `end`, and `description` (unknown keys fail with `unknown-field`):

- **Non-equivalence `entity` graphs**: allow `start`, `end`, and `description`.
  - Intervals are **half-open** `[start, end)`, or an **instant point** `[t, t]` when `start == end`.
  - Omitted `start` defaults to `-∞`; omitted `end` defaults to `+∞`. `end < start` fails with `link-end-before-start`.
  - Supports all WEFT time expressions, plus intra-block bare `start`/`end` self-references.
  - Batch edges and cliques share the trailing `{ ... }` block across all expanded pairs.
- **`event` graphs**: events already carry timestamps, so `start`/`end` are forbidden (`event-link-time-bounds`), and `equivalence` is forbidden (`event-graph-equivalence`); `description` is allowed.
- **`equivalence` graphs**: quotient identity merging is timeless, so `start`/`end` are forbidden (`equivalence-link-time-bounds`); `description` is allowed.
- **Atomic time-slice checking**: for graphs enabling `acyclic`, `asymmetric`, `intransitive`, `functional`, or `inverse_functional`, the compiler collects all edge `start`/`end` ticks in that graph, partitions the timeline into disjoint atomic half-open slices (plus instant points), and checks graph invariants independently on each slice via `petgraph`. Consequently, sequential ownership hand-offs (e.g. `end = T` followed by `start = T`) are valid under `inverse_functional = true`; only overlapping intervals conflict.
- **`--syntax-only` fallback**: when `.rn` scripts are skipped, the compiler still runs static graph constraint checks and duplicate-link checks on all timeless edges (edges without `start` or `end`).

```weft
link 组织归属 {
  acyclic    = true
  transitive = true

  格兰芬多 -> 霍格沃茨
  [哈利·波特, 罗恩·韦斯莱, 赫敏·格兰杰] -> 格兰芬多 {
    start = 开学宴分院.start
  }
}

link 物品持有 {
  inverse_functional = true

  詹姆·波特 -> 隐形衣 {
    end = [1981, 10, 31]
  }
  哈利·波特 -> 隐形衣 {
    start = 圣诞节礼物.start
  }
}

link 身份等价 {
  equivalence = true

  伏地魔 <-> 汤姆·里德尔
}

link 人物关系 {
  莉莉·波特 <- "夫妻" -> 詹姆·波特
  <- "好友" -> [哈利·波特, 罗恩·韦斯莱, 赫敏·格兰杰]
  哈利·波特 -- "怀疑" -> 斯内普 {
    start       = 开学宴分院.start
    end         = 冥想盆真相.start
    description = "一直以为斯内普在帮伏地魔偷魔法石。"
  }
}

link 剧情阻塞 {
  acyclic       = true
  chronological = true

  圣诞节礼物 -- "获得隐形衣" -> 厄里斯魔镜
}
```

______________________________________________________________________

## Time Expressions & Resolution

Time expressions are valid in `time`, `entity.base_time`, `event.start`, `event.end`, and non-equivalence `entity` `link` edge `start`/`end`:

- **Absolute time**:
  - Scalar integer `n` is shorthand for `[n]`.
  - Non-empty integer list `[c0, c1, ...]` sets components from most-significant to least-significant (for built-in Gregorian: `[year, month, day, hour, minute, second]`).
  - Length must not exceed the active calendar's `components` (`too-many-time-components`).
  - Component `0` has **no lower bound** (negative values such as `[-44, 3, 15]` are valid). For `i > 0`, any component `< minimums[i]` fails with `time-component-below-minimum` (e.g. Gregorian month or day `0`).
  - Omitted trailing components fill from `minimums` (`[0, 1, 1, 0, 0, 0]` for Gregorian) while preserving the written length as display `precision`.
- **References**:
  - Bare `time` anchor name (`t0`).
  - Member access: `ev.start`, `ev.end` (falls back to `ev.start` if `ev` has no `end`), `ent.base_time`. Quote special base names: `"v2/cutover".start`.
  - Intra-block self-reference: bare `start` or `end` inside an `event` block or a `link` edge `{ ... }` block refers to the other endpoint of that same block.
- **Relative offsets**:
  - Left-associative `+` / `-` with an integer or non-empty integer list on the right (e.g. `ev.start - [0, 0, 7] + [0, 0, 0, 2]`).
  - Offsets add/subtract component-wise, then normalize via the active calendar's `to_tick` / `from_tick` (handling carries, borrows, and leap rules). Output `precision` is `max(base_precision, offset_len)`.
- **Constraints**:
  - References must resolve (`unknown-time-ref`), cannot cycle (`time-cycle`), and cannot exceed nesting depth 64 (`time-nesting-too-deep`).

______________________________________________________________________

## Mapping Timeline Domains to WEFT

WEFT's 6 primitives are domain-agnostic. Separate **state/structure graphs** (`entity` `link` blocks) from **temporal/causal event graphs** (`event` `link` blocks):

| Domain                    | `entity`                                   | `entity` `link` graphs                                                                                                                           | `event`                                       | `event` `link` graphs (`acyclic + chronological`) | `events`                                    |
| ------------------------- | ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------- | ------------------------------------------------- | ------------------------------------------- |
| Novel / screenplay        | Characters, factions, places, items        | Org membership (`acyclic+transitive`), space (`acyclic+functional`), items (`inverse_functional`), aliases (`equivalence`), relationships (free) | Scenes, plot beats, reveals                   | Foreshadowing chains, plot prerequisites/blocking | Main plot, character POVs, subplots         |
| Incident postmortem       | Services, clusters, queues, on-call teams  | Ownership (`acyclic+transitive`), call/dependency mesh (free), active leader (`inverse_functional`)                                              | Deploy, alert, failover, mitigation, recovery | Root-cause & trigger chain                        | Full timeline, root-cause chain             |
| Project / product roadmap | Teams, epics, services, milestones         | Org/epic tree (`acyclic+functional`), service deps (free)                                                                                        | Kickoff, release windows, cutovers, launches  | Hard milestone blockers / dependencies            | Release train, per-team tracks              |
| Historical chronology     | Polities, figures, institutions, artifacts | Vassalage/subdivision (`acyclic+transitive`), succession/territories (temporal), alliances (`symmetric`)                                         | Treaties, battles, reigns, discoveries        | Historical cause-and-effect chains                | Global chronology, regional threads         |
| Tech / org evolution      | Systems, squads, architectures, RFCs       | Team ownership (`acyclic+transitive`), replacements/aliases (`equivalence`), integrations (free)                                                 | Launches, deprecations, migrations, re-orgs   | Prerequisite migrations & rollouts                | Architecture evolution, platform milestones |

______________________________________________________________________

## Diagnostics & Lint Reference

### Compile Errors (exit code 1)

#### Syntax & Declarations

| Rule                                                                                                           | Trigger & Fix                                                                                             |
| -------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| `syntax-error`                                                                                                 | Missing `}` / `]`, missing `=`, unclosed string, unquoted special name or edge label -> fix around span.  |
| `missing-story`                                                                                                | No `story { ... }` block -> add one with `title = "..."`.                                                 |
| `duplicate-story`                                                                                              | More than one `story` block -> merge into a single `story` block.                                         |
| `duplicate-entity` / `duplicate-event` / `duplicate-events` / `duplicate-link-graph` / `duplicate-time-anchor` | Duplicate top-level declaration name of the same kind -> rename or merge.                                 |
| `duplicate-symbol`                                                                                             | An `entity` and an `event` share the same name -> rename one (unified node namespace).                    |
| `missing-field`                                                                                                | Required field missing (`story.title`, `event.start`) -> add it.                                          |
| `unknown-field`                                                                                                | Disallowed key in `story`, `event`, `link` graph, or `link` edge block -> remove or fix typo.             |
| `invalid-field`                                                                                                | Wrong value type (e.g. unquoted string field, non-boolean graph flag, expression in `entity` extra prop). |

#### Link Graph Configuration & Algebraic Checks

| Rule                           | Trigger & Fix                                                                                                                                               |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `invalid-graph-config`         | Mutually exclusive graph flags enabled together -> remove conflicting flag.                                                                                 |
| `mixed-link-node-kinds`        | Same `link` graph connects both `entity` and `event` nodes -> split into separate homogeneous graphs.                                                       |
| `event-graph-equivalence`      | `equivalence = true` on an `event` graph -> remove `equivalence` (entity-only).                                                                             |
| `event-link-time-bounds`       | Edge in an `event` graph specifies `start` or `end` -> remove `start`/`end`.                                                                                |
| `equivalence-link-time-bounds` | Edge in an `equivalence` graph specifies `start` or `end` -> remove `start`/`end`.                                                                          |
| `invalid-clique`               | Clique `<-> [...]` has fewer than 2 members -> add members or use a regular edge.                                                                           |
| `empty-link-endpoint`          | Batch endpoint list is `[]` -> populate or remove the edge.                                                                                                 |
| `arrow-direction-mismatch`     | Directed edge in `symmetric`/`equivalence` graph, or undirected/clique in `acyclic`/`asymmetric`/`chronological` graph -> fix arrow or graph flags.         |
| `graph-cycle`                  | Directed cycle or self-loop in an `acyclic` graph during an atomic time slice -> break cycle or adjust edge intervals.                                      |
| `asymmetry-conflict`           | Bidirectional edges (`A -> B` and `B -> A`) or self-loop in an `asymmetric` graph during an overlapping slice -> remove reverse edge or separate intervals. |
| `intransitivity-violation`     | Shortcut `A -> C` coexists with `A -> B -> C` in an `intransitive` graph during a slice -> remove shortcut or separate intervals.                           |
| `functional-conflict`          | Source node has `> 1` distinct outgoing targets in a `functional` graph during a slice -> tighten `start`/`end` or drop extra edge.                         |
| `inverse-functional-conflict`  | Target node has `> 1` distinct incoming sources in an `inverse_functional` graph during a slice -> tighten `start`/`end` or drop extra edge.                |
| `chronological-violation`      | Directed edge `from -> to` in a `chronological` graph has `from` time `> to` time -> reverse direction or fix timestamps.                                   |

#### Time, References & Plugins

| Rule                                                                             | Trigger & Fix                                                                                                              |
| -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `unknown-time-ref`                                                               | Unknown anchor, event, entity, or field (only `.start`, `.end`, `.base_time` exist).                                       |
| `time-cycle`                                                                     | Circular time references (e.g. `A.start` depends on `B.start` depends on `A.start`).                                       |
| `time-nesting-too-deep`                                                          | Time anchor/offset chain exceeds depth 64 -> flatten intermediate anchors.                                                 |
| `event-end-before-start` / `link-end-before-start`                               | Resolved `end` tick is earlier than `start` tick -> fix `end` or offset sign.                                              |
| `time-nonempty-list` / `time-integer-component` / `time-offset-out-of-range`     | Empty `[]`, non-integer component, or integer overflow in time expression.                                                 |
| `too-many-time-components` / `time-component-below-minimum`                      | More components than calendar allows, or component `i > 0` below `minimums[i]`.                                            |
| `unknown-link-entity` / `unknown-event-entity` / `unknown-events-event`          | Reference to undeclared node/entity/event (diagnostics include spelling suggestions).                                      |
| `invalid-avatar-path`                                                            | Avatar path missing or unsupported image extension (`bmp/gif/ico/jpeg/jpg/png/svg/webp`).                                  |
| `invalid-calendar-path` / `calendar-*` / `normalize-wrong-component-count`       | Calendar `.rn` path missing, compile/runtime/interface/round-trip error; see [`rune-plugins.md`](rune-plugins.md).         |
| `invalid-calculable-prop-path` / `unknown-calculable-prop` / `calculable-prop-*` | Calculable prop `.rn` missing, unregistered ID, or compile/runtime/return error; see [`rune-plugins.md`](rune-plugins.md). |

### Lint Warnings (exit code 1 under `--deny-warnings`)

| Rule                 | Trigger & Fix                                                                                                                                                  |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `duplicate-attr`     | Same key repeated within one block (later value overwrites earlier) -> remove duplicate.                                                                       |
| `unused-time-anchor` | Declared `time` anchor is never referenced -> use or delete it.                                                                                                |
| `unused-entity`      | File has `>= 1` event or `link` edge, and this entity is referenced by none -> connect or remove it.                                                           |
| `unreferenced-event` | File declares `>= 1` `events` list, and this event is in none -> add it to an `events` list.                                                                   |
| `self-link`          | Edge connects a node to itself (suppressed when `acyclic`/`asymmetric` already emits an error) -> fix endpoint or delete.                                      |
| `duplicate-link`     | Same graph, endpoints, direction, label, and overlapping time interval (endpoint-order-insensitive for undirected edges) -> deduplicate or separate intervals. |
| `empty-block`        | `link` block with no edges, or empty `events` list `[]` -> populate or delete.                                                                                 |

______________________________________________________________________

## Formatting Specification (`weft fmt`)

`weft fmt` is strictly idempotent, semantic-preserving, and refuses files containing syntax errors:

- **Indentation & spacing**: 2-space indentation; one blank line between top-level declarations; at most one blank line between attribute groups or edges.
- **`=` alignment**: aligns `=` by Unicode display width within each contiguous attribute group (blank lines or standalone comment lines start a new alignment group; trailing inline comments do not break alignment).
- **`link` block layout**: block-level graph attributes are grouped and `=`-aligned at the top, followed by a blank line, then edge and clique statements. Arrow operators (`->`, `<->`, `-- "label" ->`, `<- "label" ->`) keep single spaces around operators; edge labels are always double-quoted; trailing `{ ... }` edge blocks follow standard attribute-block formatting.
- **List wrapping**:
  - **Name lists** (`events`, clique member lists, `link` batch endpoint lists): stay single-line if comment-free, `<= 4` items, and fitting within 88 columns; otherwise expand one element per line with trailing commas.
  - **Expression lists** (e.g. `entities`, `calculable_props`, time tuples, extra prop arrays): stay single-line if comment-free and fitting within 88 columns; otherwise expand one element per line with trailing commas.
- **Strings & names**: multi-line strings that round-trip losslessly through `dedent_multiline` format as `"""..."""`; all other strings format as single-line escaped `"..."`. Names are quoted only when not valid bare identifiers.
- **Comments**: header, standalone, inline, and block comments are preserved.
