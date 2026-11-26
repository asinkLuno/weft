# Weft Syntax, Modeling & Diagnostics Reference

Complete reference for authoring `.weft` files. Use alongside `weft check` and `weft fmt`.

## Contents

1. [Lexical Rules](#lexical-rules)
2. [Top-Level Declarations](#top-level-declarations)
3. [Time Expressions](#time-expressions)
4. [Modeling Patterns](#modeling-patterns)
5. [Diagnostics & Lint Troubleshooting](#diagnostics--lint-troubleshooting)
6. [Formatting Rules](#formatting-rules)

---

## Lexical Rules

- **Comments**: `# ...` and `// ...` to end of line; `/* ... */` across lines. Preserved by `weft fmt`.
- **Bare names**: Start with Unicode letter or `_`; continue with letter, digit, `_`, `·`, `・`. Cannot be `true`, `false`, or `null`.
  - Valid bare: `哈利·波特`, `event_1`, `霍格沃茨魔法学校`
  - Must quote: `"9又3/4站台"`, `"Chapter 1"`, `"2024-事件"`, `"true"`
- **Strings**:
  - Single-line `"..."` supports `\\`, `\"`, `\n`, `\r`, `\t`.
  - Multi-line `"""..."""` strips leading/trailing boundary newlines and common indentation across non-empty lines. Best for multi-paragraph `description`.
- **Numbers & literals**: Integers (`42`), floats (`3.14`), `true`, `false`, `null`. Negative signs apply in expressions (`-1`), not inside bare absolute time components (except year as the first component, which has no lower bound).
- **Collections**: Lists `[a, b]` and objects `{ k = v }`. Elements separated by newlines, commas, or semicolons; trailing commas allowed.
- **Bare identifiers vs strings**:
  - Name lists (`belong_to`, `entities`, `calculable_props`, `narrative`) treat bare identifiers as names.
  - Typed string fields (`title`, `author`, `description`, `avatar`, `date_mode`) strictly require quoted strings.
  - In `entity` extra fields, a bare identifier evaluates as a string value, though quoting strings explicitly is clearer.

---

## Top-Level Declarations

### `story { ... }` (required, exactly one)

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `title` | Yes | String | Story title |
| `author` | No | String | Author name |
| `description` | No | String | Synopsis |
| `date_mode` | No | String | Relative path to `.rn` calendar script; defaults to built-in Gregorian |
| `calculable_props` | No | Object `{ id = "path.rn" }` | Registry mapping prop IDs to relative `.rn` script paths |

Unknown fields in `story` are compile errors (`unknown-field`).

### `time <name> = <expr>`

Declares a reusable time anchor usable in `base_time`, `start`, `end`, or another `time` anchor. Unused anchors trigger `unused-time-anchor`.

### `entity <name> { ... }`

Represents characters, factions, locations, artifacts, or any story concept. Names must be globally unique.

| Field | Type | Notes |
| --- | --- | --- |
| `description` | String | Entity description |
| `base_time` | Time expr | Birth/founding/origin time |
| `avatar` | String | Relative path to existing `.bmp`/`.gif`/`.ico`/`.jpeg`/`.jpg`/`.png`/`.svg`/`.webp` |
| `belong_to` | List of entity names | Parent entities; forms a DAG (multi-parent allowed, cycles forbidden) |
| `calculable_props` | List of prop IDs | IDs must be registered in `story.calculable_props`; computed results merge into `extra_props` |
| *any other key* | Scalar / list / object | Passed through verbatim to `extra_props` (cannot use time refs or `+`/`-`) |

### `link <group> { ... }`

Groups named relationships between declared entities:

```weft
link 师徒 {
  师父 -- "授业" -- 徒弟
}
```

- Syntax: `<entity> -- "<relation>" -- <entity>`.
- Both endpoints must be declared entities.
- Multiple `link` blocks with the same `<group>` name merge their edges.
- `fmt` always quotes `<relation>`.

### `event <name> { ... }`

Names must be globally unique. Unknown fields are errors (`unknown-field`).

| Field | Required | Type | Notes |
| --- | --- | --- | --- |
| `start` | Yes | Time expr | Start coordinate |
| `end` | No | Time expr | End coordinate; must satisfy `end >= start` (by calendar tick) |
| `entities` | No | List of entity names | Participating entities |
| `description` | No | String | Event summary |

### `narrative <name> = [event1, event2, ...]`

Ordered list of declared event names defining a reading/viewing order (chronological, POV thread, flashback order, etc.). Repeating an event in a narrative is allowed.

---

## Time Expressions

Valid in `time`, `entity.base_time`, `event.start`, and `event.end`:

1. **Absolute scalar or list**:
   - `1991` is shorthand for `[1991]`.
   - `[1991, 9, 1, 19, 30, 0]` for built-in Gregorian `[year, month, day, hour, minute, second]`.
   - Omitted trailing components default to the calendar's `minimums` (`[0, 1, 1, 0, 0, 0]` for Gregorian).
   - Components after the first cannot be smaller than `minimums[i]` (`time-component-below-minimum`).
   - Length cannot exceed `calendar.components` (`too-many-time-components`).
2. **References**:
   - Time anchor: `开学`
   - Event start/end: `分院.start`, `分院.end`, `"第1章：启程".start`
   - Entity base time: `哈利·波特.base_time`
   - Self-reference inside an `event`: `start` or `end` (e.g. `end = start + [0, 0, 0, 2]`)
3. **Offsets (`+` / `-`)**:
   - Right-hand side must be an integer or integer list, e.g. `start + [0, 0, 7]` (+7 days) or `开学 - [0, 1]` (-1 month).
   - Chainable (`anchor + [0, 0, 2] - [0, 0, 0, 3]`).
   - Calendar normalizes carries/borrows automatically via `from_tick(to_tick(values))`.
   - Result precision becomes `max(base_precision, offset_length)`.
4. **Limits**: No reference cycles (`time-cycle`), max nesting depth 64 (`time-nesting-too-deep`).

---

## Modeling Patterns

- **Classify entities with a custom `kind` field**: Since `entity` models people, places, organizations, and items uniformly, adding `kind = "人物"` / `"地点"` / `"组织"` / `"物品"` (or English equivalents) makes large documents much clearer in the UI.
- **Use `belong_to` for hierarchical membership (DAG)**: Organizations -> sub-teams -> people, or regions -> cities -> buildings. An entity can belong to multiple parents (e.g., `[格兰芬多, 霍格沃茨魔法学校]`). Use `link` for peer/typed relations (family, rivalry, mentorship) rather than forcing them into `belong_to`.
- **Anchor shared timelines with `time`**: When several events happen relative to a pivotal moment (e.g. `time 启航 = [2150, 3, 15]`), define a `time` anchor and derive events with `+`/`-` offsets so shifting the anchor shifts the sequence consistently.
- **Multiple narratives**: Create a main `narrative` plus optional POV or subplot narratives (e.g. `narrative 主线 = [...]`, `narrative 暗线 = [...]`). Remember that once any `narrative` exists, every `event` should appear in at least one to avoid `unreferenced-event`.
- **Section comments**: Group declarations with `# ---- 实体 ----`, `# ---- 实体关系 ----`, `# ---- 事件 ----`, `# ---- 叙事线 ----`. `weft fmt` preserves comments cleanly.

---

## Diagnostics & Lint Troubleshooting

### Compile Errors (exit code 1)

| Rule | Cause & Fix |
| --- | --- |
| `syntax-error` | Missing `}`/`]`/`=`, unclosed string, invalid bare name (quote names with spaces/punctuation/leading digits). |
| `missing-story` / `duplicate-story` | Ensure exactly one `story { ... }` block exists. |
| `duplicate-entity` / `duplicate-event` / `duplicate-narrative` / `duplicate-time-anchor` | Rename colliding top-level declarations. |
| `missing-field` | Add required `story.title` or `event.start`. |
| `unknown-field` | Remove unsupported key from `story` or `event` (only `entity` supports extra keys). |
| `invalid-field` | Wrong value type (most often an unquoted bare identifier in `description`/`title`/`date_mode`, or a time expression in an `entity` extra prop). |
| `unknown-time-ref` | Typo in anchor/event/entity name, or referencing `.end` on an event without `end`, or `.base_time` on an entity without `base_time`. |
| `time-cycle` / `time-nesting-too-deep` | Break cyclic time references; keep chain depth <= 64. |
| `event-end-before-start` | Ensure `end` resolves to a tick >= `start`. |
| `time-nonempty-list` / `time-integer-component` / `too-many-time-components` / `time-component-below-minimum` | Fix malformed time list, non-integer elements, excess components, or sub-minimum values (like month `0`). |
| `unknown-link-entity` / `unknown-event-entity` / `unknown-narrative-event` / `entity-unknown-parent` | Declare the referenced entity/event or fix typo (CLI prints `did you mean` hints). |
| `entity-cycle` | Break cycle in `belong_to` chains. |
| `invalid-avatar-path` | Check relative path from the `.weft` file and image extension (`bmp/gif/ico/jpeg/jpg/png/svg/webp`). |
| `invalid-calendar-path` / `invalid-calculable-prop-path` / `unknown-calculable-prop` | Verify `.rn` file exists relative to `.weft`, ends with `.rn`, and ID is registered in `story.calculable_props`. |
| `calendar-*` / `calculable-prop-*` | Rune script compile/interface/runtime/return error; see `references/rune-plugins.md`. |

### Lint Warnings (fail under `--deny-warnings`)

| Rule | Trigger & Fix |
| --- | --- |
| `duplicate-attr` | Same key appears twice in one block -> remove duplicate. |
| `unused-time-anchor` | `time` anchor never referenced -> use it or delete it. |
| `unused-entity` | Entity not referenced by any `event`, `link`, or `belong_to` -> connect it or remove dead entity. |
| `unreferenced-event` | Document has narratives, but this event is in none -> add it to a `narrative`. |
| `self-link` | `A -- "rel" -- A` -> remove or fix target entity. |
| `duplicate-link` | Identical `(left, relation, right)` edge repeated in the same link group -> deduplicate. |
| `empty-block` | Empty `link` or `narrative` -> populate or delete. |

---

## Formatting Rules (`weft fmt`)

`weft fmt` is idempotent and comment-preserving:
- 2-space indentation; at most one blank line between top-level items or attribute groups.
- Aligns `=` by display width within each contiguous attribute group (blank lines or standalone comments start a new alignment group).
- Lists stay single-line if within 88 columns and comment-free; otherwise expand to one item per line with trailing commas.
- `narrative` stays single-line when <= 4 events and <= 88 columns; otherwise multiline.
- Refuses to rewrite files containing syntax errors.
