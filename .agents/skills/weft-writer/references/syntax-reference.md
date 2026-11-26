# Weft Syntax, Semantics & Diagnostics Reference

Complete language reference for authoring, debugging, and formatting `.weft` files with `weft check` and `weft fmt`.

## Contents

1. [Lexical Rules](#lexical-rules)
2. [Top-Level Declarations](#top-level-declarations)
3. [Time Expressions & Resolution](#time-expressions--resolution)
4. [Mapping Timeline Domains to Weft](#mapping-timeline-domains-to-weft)
5. [Diagnostics & Lint Reference](#diagnostics--lint-reference)
6. [Formatting Specification (`weft fmt`)](#formatting-specification-weft-fmt)

---

## Lexical Rules

- **Comments**:
  - Line comments: `# ...` and `// ...` until end of line.
  - Block comments: `/* ... */` across lines.
  - Comment markers inside string literals are literal text. `weft fmt` preserves comments.
- **Names (identifiers)**:
  - **Bare name**: starts with a Unicode alphabetic character or `_`; continues with Unicode alphanumeric characters, `_`, `·`, or `・`. Cannot be `true`, `false`, or `null`.
    - Valid bare: `平台组`, `node_1`, `亚历山大·弗莱明`, `T0`
    - Must be double-quoted: `"api/v2"`, `"2026-Q1"`, `"Phase 1"`, `"502-网关"`, `"true"`
  - Anywhere a `<name>` is expected (`entity`, `link` group/endpoints/relation, `event`, `events`, attribute keys, member access base), either a bare name or a double-quoted string is parsed—**except** referencing a `time` anchor inside a time expression, which only resolves bare identifiers (`Expr::Ident`). Always declare `time` anchors with bare names.
- **Strings**:
  - Single-line `"..."` supports escapes `\\`, `\"`, `\n`, `\r`, `\t`.
  - Multi-line `"""..."""` strips the opening/closing boundary newlines and removes common leading indentation across non-empty lines. Ideal for multi-line `description` blocks.
- **Numbers & literals**:
  - Signed integers (`42`, `-44`), signed floats (`3.14`, `-0.5`), booleans (`true`, `false`), `null`.
- **Collections & separators**:
  - **Lists** `[e1, e2, ...]` and **`events` lists** `[ev1, ev2, ...]`: elements **must** be separated by `,` (trailing comma allowed).
  - **Attribute blocks / objects** `{ k = v ... }`: attributes are separated by newlines, spaces, or optional `,` / `;`. Keys can be bare or quoted names.
- **Bare identifier vs quoted string semantics**:
  - Typed string fields (`story.title`, `story.author`, `story.description`, `story.date_mode`, `entity.description`, `entity.avatar`, `event.description`) strictly require string literals (`"..."` / `"""..."""`). Bare identifiers fail with `invalid-field`.
  - Name lists (`belong_to`, `entities`, `calculable_props`, `events`) resolve both bare and quoted names as symbol references.
  - In `entity` extra fields (`extra_props`), a bare identifier evaluates to a string value, though quoting strings explicitly is clearer.

---

## Top-Level Declarations

Top-level items may appear in any order; forward references across `time`, `entity`, `link`, `event`, and `events` are resolved during compilation.

### `story { ... }` (required, exactly one)

Document header and plugin registry.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `title` | Yes | String | Document / timeline title |
| `author` | No | String | Author or owner |
| `description` | No | String | Overview / summary |
| `date_mode` | No | String | Relative path to `.rn` calendar / time-axis script; defaults to built-in Gregorian |
| `calculable_props` | No | Object `{ id = "path.rn" }` | Maps prop IDs to relative `.rn` script paths |

Unknown fields in `story` are compile errors (`unknown-field`).

### `time <name> = <time_expr>`

Declares a reusable named coordinate anchor usable in `entity.base_time`, `event.start`, `event.end`, or another `time` anchor.

```weft
time 故障起点 = [2026, 3, 15, 14, 22]
time 恢复窗口 = 故障起点 + [0, 0, 0, 1]
```

- Declare `<name>` as a bare identifier so time expressions can reference it.
- Unused anchors emit `unused-time-anchor`.

### `entity <name> { ... }`

Models any subject, node, component, person, organization, place, artifact, or concept on the timeline. Names must be globally unique (`duplicate-entity`).

| Field | Type | Notes |
| --- | --- | --- |
| `description` | String | Entity summary |
| `base_time` | Time expr | Origin, creation, birth, or launch coordinate |
| `avatar` | String | Relative path to an existing `.bmp`/`.gif`/`.ico`/`.jpeg`/`.jpg`/`.png`/`.svg`/`.webp` file |
| `belong_to` | List of entity names | Parent entities forming a DAG (multiple parents allowed; cycles fail with `entity-cycle`) |
| `calculable_props` | List of prop IDs | IDs registered in `story.calculable_props`; outputs merge into `extra_props` |
| *any other key* | Scalar / list / object | Stored verbatim in `extra_props` (time refs and `+`/`-` expressions are rejected here) |

### `link <group> { ... }`

Groups named relationships between declared entities:

```weft
link 架构依赖 {
  "api/v2" -- "读写" -- 主数据库
  "api/v2" -- "缓存" -- Redis集群
}
```

- Edge grammar: `<name> -- <name> -- <name>` (`left -- relation -- right`).
- Both endpoints must be declared entities (`unknown-link-entity`).
- Multiple `link` blocks sharing the same `<group>` name merge their edges.
- `weft fmt` always quotes `<relation>`.

### `event <name> { ... }`

Models a point-in-time occurrence or time interval on the timeline. Names must be globally unique (`duplicate-event`). Unknown keys fail with `unknown-field`.

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `start` | Yes | Time expr | Start coordinate |
| `end` | No | Time expr | End coordinate; must satisfy `end >= start` by calendar tick (`event-end-before-start`) |
| `entities` | No | List of entity names | Entities participating in or affected by the event |
| `description` | No | String | Event details |

### `events <name> = [event1, event2, ...]`

Declares an ordered list of event names representing a track, reading order, causal slice, or filtered view.

- All listed events must be declared (`unknown-events-event`).
- Repeating an event inside an `events` list is valid.
- When at least one `events` declaration exists, any declared `event` absent from every `events` list triggers `unreferenced-event`.

---

## Time Expressions & Resolution

Time expressions are valid only in `time`, `entity.base_time`, `event.start`, and `event.end`.

### 1. Absolute Coordinates

- Scalar integer `n` is shorthand for `[n]` (precision `1`).
- Non-empty integer list `[c0, c1, ...]` sets components from most-significant to least-significant; precision equals the list length.
- Built-in Gregorian calendar has 6 components `[year, month, day, hour, minute, second]` with `minimums = [0, 1, 1, 0, 0, 0]`.
- Omitted trailing components fill from `minimums` (`[2026, 3]` -> `[2026, 3, 1, 0, 0, 0]`, precision `2`).
- Component `0` has **no lower bound** (negative values such as `[-44, 3, 15]` for 44 BCE or negative ticks are valid).
- Components `i > 0` cannot be less than `minimums[i]` (`time-component-below-minimum`).
- List length cannot exceed `calendar.components` (`too-many-time-components`).

### 2. References

| Form | Target | Quoting rule |
| --- | --- | --- |
| `anchor_name` | `time anchor_name` | Must be a bare identifier |
| `ev.start` / `"ev name".start` | `event`'s `start` | Base may be bare or quoted |
| `ev.end` / `"ev name".end` | `event`'s `end` (must exist) | Base may be bare or quoted |
| `ent.base_time` / `"ent".base_time` | `entity`'s `base_time` (must exist) | Base may be bare or quoted |
| `start` / `end` (inside an `event`) | Current event's own `start` / `end` | Bare `start` / `end` only |

### 3. Relative Offsets (`+` / `-`)

- Left-associative binary operators `+` and `-`; right-hand side must be an integer or non-empty integer list:
  - `start + [0, 0, 7]` (+7 days)
  - `割接窗口 - [0, 0, 0, 1] + [0, 0, 0, 0, 30]` (-1 hour, +30 minutes)
  - `epoch + 5` (+5 in component 0)
- Offsets add/subtract component-wise, then normalize through the calendar via `from_tick(to_tick(values))`, carrying and borrowing automatically across boundaries.
- Resulting precision is `max(base_precision, offset_length)`.
- Reference cycles fail (`time-cycle`); maximum nesting depth is `64` (`time-nesting-too-deep`).

---

## Mapping Timeline Domains to Weft

Because the grammar provides general graph + timeline primitives, any time-indexed domain maps cleanly without changing syntax:

| Domain | `entity` (+ `belong_to` DAG) | `link` | `event` | `events` | Optional `.rn` plugin |
| --- | --- | --- | --- | --- | --- |
| **Incident postmortem** | Services, infra clusters, teams, on-call roles | Traffic/data dependencies, escalation paths | Alerts, mitigations, rollbacks, recovery windows | Full timeline, root-cause chain, customer impact | Elapsed incident clock `[Hour, Min, Sec]` |
| **Project / product roadmap** | Teams, workstreams, modules, deliverables | Blocking / upstream dependencies | Sprints, launches, freezes, milestones | Release track, per-team track, critical path | `[Year, Quarter, Week]` or `[Release, Sprint, Day]` |
| **Historical chronology** | Polities, institutions, figures, treaties, regions | Alliances, rivalries, successions | Battles, reforms, eras, discoveries | Global chronology, regional or thematic threads | Historical era / lunar calendar |
| **Biography / org chronicle** | Person, companies, schools, projects, awards | Mentors, co-founders, affiliations | Education, roles, publications, transitions | Career arc, research thread | Derived age/tenure/phase prop |
| **Fiction / script** | Characters, factions, locations, items | Family, mentorship, conflicts | Scenes, plot beats, arcs | Chronological order, POV or flashback order | Fictional world calendar |

Tip: When an `entity` set mixes heterogeneous types (e.g. teams, services, and regions), add a consistent custom attribute like `kind = "service"` or `category = "team"` in `extra_props` so consumers can filter and group cleanly.

---

## Diagnostics & Lint Reference

### Compile Errors (exit code 1)

| Rule | Cause & Fix |
| --- | --- |
| `syntax-error` | Missing `}`/`]`/`=`/`,` in a list, unclosed string, or invalid bare name (quote names with spaces, `/`, `-`, punctuation, or leading digits). |
| `missing-story` / `duplicate-story` | Ensure exactly one `story { ... }` block exists. |
| `duplicate-entity` / `duplicate-event` / `duplicate-events` / `duplicate-time-anchor` | Rename colliding top-level declarations. |
| `missing-field` | Add required `story.title` or `event.start`. |
| `unknown-field` | Remove unsupported key from `story` or `event` (only `entity` accepts arbitrary extra keys). |
| `invalid-field` | Wrong value type (most commonly an unquoted bare identifier in a string field, or a time expression in `entity` extra props). |
| `unknown-time-ref` | Typo in anchor/event/entity name, quoting a `time` anchor name, or referencing `.end` / `.base_time` when not defined on the target. |
| `time-cycle` / `time-nesting-too-deep` | Break cyclic time references; keep offset/ref depth `<= 64`. |
| `event-end-before-start` | Ensure `end` resolves to a tick `>= start`. |
| `time-nonempty-list` / `time-integer-component` / `too-many-time-components` / `time-component-below-minimum` / `time-offset-out-of-range` | Fix empty/malformed time list, non-integer elements, excess components, values `< minimums[i]` (for `i > 0`), or integer negation overflow. |
| `unknown-link-entity` / `unknown-event-entity` / `unknown-events-event` / `entity-unknown-parent` | Declare the referenced entity/event or fix typo (CLI prints `did you mean` hints). |
| `entity-cycle` | Break cycles in `belong_to` chains. |
| `invalid-avatar-path` | Check relative path from the `.weft` file and image extension (`bmp/gif/ico/jpeg/jpg/png/svg/webp`). |
| `invalid-calendar-path` / `invalid-calculable-prop-path` / `unknown-calculable-prop` | Verify `.rn` file exists relative to `.weft`, ends with `.rn`, and ID is registered in `story.calculable_props`. |
| `calendar-*` / `calculable-prop-*` | Rune script compile/interface/runtime/return/round-trip error; see [`rune-plugins.md`](rune-plugins.md). |

### Lint Warnings (exit code 1 under `--deny-warnings`)

| Rule | Trigger & Fix |
| --- | --- |
| `duplicate-attr` | Same key repeated in one block -> remove duplicate. |
| `unused-time-anchor` | Declared `time` anchor never referenced -> use or delete it. |
| `unused-entity` | Entity unreferenced by any `event`, `link`, or `belong_to` (checked when the file has `>= 1` event or link) -> connect or remove it. |
| `unreferenced-event` | File has `>= 1` `events`, but this event is in none -> add it to an `events` list. |
| `self-link` | `A -- "rel" -- A` -> remove or fix target entity. |
| `duplicate-link` | Same `(left, relation, right)` edge repeated within a `link` group -> deduplicate. |
| `empty-block` | Empty `link` block or empty `events` list -> populate or delete. |

---

## Formatting Specification (`weft fmt`)

`weft fmt` is idempotent, semantic-preserving, and refuses files containing syntax errors:

- 2-space indentation; at most one blank line between top-level declarations or attribute groups.
- Aligns `=` by display width within each contiguous attribute group (blank lines or standalone comment lines start a new group; trailing inline comments do not break alignment).
- Lists stay single-line if `<= 88` columns and comment-free; otherwise expand one element per line with trailing commas.
- `events` stays single-line when `<= 4` events and `<= 88` columns; otherwise multiline.
- Multi-line strings format as `"""..."""`; single-line strings normalize escapes.
- Names are quoted only when required; `link` relations are always double-quoted.
- Comments (header, standalone, inline) are preserved.
