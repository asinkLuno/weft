---
name: weft-writer
description: >
  Author, edit, validate, and format `.weft` story documents and accompanying `.rn` Rune
  plugins (custom calendars via `date_mode` and calculable properties via `calculable_props`),
  using the `weft` CLI (`weft check`, `weft fmt`, `weft rules`) to enforce syntax, semantic,
  script, and lint correctness. Trigger this skill whenever the user wants to write a `.weft`
  file, model a story/characters/relationships/timeline/narratives in Weft, convert story notes
  or outlines into Weft, write or debug `.rn` calendar or calculable-prop scripts for Weft,
  or lint/format/fix diagnostics in `.weft` documents—even if they don't explicitly mention
  "weft check" or "weft fmt".
---

# Weft Story Authoring & Validation

Write clean, semantically valid `.weft` story documents (and optional `.rn` Rune plugins), then verify and format them with the `weft` CLI until both `check --deny-warnings` and `fmt --check` pass.

## Workflow

1. **Plan the story model** before writing:
   - Identify `story` metadata (plus any `date_mode` / `calculable_props` `.rn` scripts).
   - Define `entity` hierarchy (`belong_to` must be a DAG, no cycles) and custom metadata (`extra_props`).
   - Group entity relationships in `link` blocks, reusable dates in `time` anchors, plot beats in `event` blocks, and reading orders in `narrative` lists.
2. **Write `.weft` (and `.rn` scripts if needed)**:
   - For `.rn` custom calendars or calculable properties, read [`references/rune-plugins.md`](references/rune-plugins.md) first so your exported signatures, context shapes, and round-trip invariants match the Rust host.
   - For detailed syntax, time math, modeling patterns, or unfamiliar diagnostic codes, read [`references/syntax-and-modeling.md`](references/syntax-and-modeling.md).
3. **Validate & format in a tight loop** (see CLI section below):
   - Always run the full `check --deny-warnings` (without `--syntax-only`) before finishing so `.rn` scripts, time resolution, avatars, and lint warnings are all exercised.
   - Run `fmt` after syntax errors are resolved—`fmt` is idempotent, preserves comments, aligns `=`, and normalizes quoting.
   - Fix every error and lint warning (don't silence warnings with `--ignore` unless the user explicitly asks to keep intentionally unused draft stubs).

## Calling the `weft` CLI

Use the bundled wrapper script `scripts/run_weft.sh` (relative to this skill directory). It automatically resolves `weft` in priority order: global `weft` on `PATH` -> `<repo>/target/release/weft` -> `<repo>/target/debug/weft` -> `cargo run -p weft-cli -q --`.

```bash
WEFT="<path-to-this-skill>/scripts/run_weft.sh"

# 1. Fast feedback while drafting (skips .rn execution and avatar existence check)
"$WEFT" check path/to/story.weft --syntax-only --output-format concise

# 2. Full check: syntax + semantics + .rn execution + lint (warnings fail)
"$WEFT" check path/to/story.weft --deny-warnings

# 3. Auto-format in place (refuses files with syntax errors; preserves comments)
"$WEFT" fmt path/to/story.weft

# 4. Final gate before replying to the user
"$WEFT" check path/to/story.weft --deny-warnings && "$WEFT" fmt --check path/to/story.weft
```

Useful flags:
- `--output-format full|concise|github|json` (`concise` or `full` works best for fixing diagnostics).
- `--lang zh|en` (sets locale passed to `.rn` calendar/calculable-prop scripts; test both if writing bilingual `.rn` scripts!).
- `rules` prints all 7 lint rule names.

## Minimal `.weft` Skeleton

```weft
story {
  title       = "示例故事"
  author      = "作者名"
  description = """
    多行简介会自动去除公共缩进。
  """
  # date_mode        = "calendar.rn"
  # calculable_props = { constellation = "constellation.rn" }
}

time 开端 = [2024, 1, 1, 9]

entity 组织 {
  description = "一个团体实体。"
}

entity 主角 {
  description = "故事主角。"
  base_time   = [2000, 5, 12]
  belong_to   = [组织]
  role        = "主角" # entity 允许任意表外字段，原样进入 extra_props
}

entity 伙伴 {
  description = "主角的同伴。"
  belong_to   = [组织]
}

link 主要关系 {
  主角 -- "搭档" -- 伙伴
}

event 相遇 {
  start       = 开端
  end         = start + [0, 0, 0, 2]
  entities    = [主角, 伙伴]
  description = "两人在清晨相遇。"
}

narrative 主线 = [相遇]
```

## High-Impact Rules & Common Pitfalls

Knowing these upfront prevents the most frequent compiler and lint errors:

- **Quote string fields**: `title`, `author`, `description`, `avatar`, `date_mode` require string literals (`"..."` or `"""..."""`). Bare identifiers fail with `invalid-field`.
- **Identifier quoting**: Bare names start with a letter or `_`, followed by letters, digits, `_`, `·`, `・` (Chinese names like `哈利·波特` need no quotes). Names with spaces, `/`, `-`, punctuation, leading digits, or matching `true`/`false`/`null` must be double-quoted, e.g. `entity "9又3/4站台"` and `"穿越9又3/4站台".start`.
- **Extra fields only on `entity`**: Arbitrary custom keys (`kind`, `wand`, `tags = [...]`, nested `{ ... }`) are valid on `entity` (collected into `extra_props`, no time refs or `+/-` ops allowed). `story` and `event` reject unknown keys with `unknown-field`.
- **Absolute time bounds**: Built-in Gregorian calendar has 6 components `[year, month, day, hour, minute, second]` with minimums `[0, 1, 1, 0, 0, 0]`. Writing `[2024, 0, 1]` or `[2024, 1, 0]` fails (`time-component-below-minimum`). Trailing omitted components fill from `minimums` (`[2024, 3]` -> March 1, 00:00:00).
- **Relative time arithmetic**: Time fields (`base_time`, `start`, `end`, `time` anchors) support references (`anchor`, `ev.start`, `ev.end`, `ent.base_time`, and intra-event `start`/`end`) plus `+`/`-` integer or list offsets (e.g. `start + [0, 0, 1]` = +1 day). Offsets carry/borrow automatically through the calendar. Keep `end >= start` and avoid reference cycles.
- **Keep lint clean (7 rules)**:
  - `unused-entity`: Every `entity` should appear in at least one `event.entities`, `link`, or another entity's `belong_to` (note: only checked when the doc has at least one event or link).
  - `unreferenced-event`: When any `narrative` exists, include every `event` in at least one `narrative`.
  - `unused-time-anchor`: Only declare `time` anchors that are actually referenced.
  - `self-link`, `duplicate-link`, `duplicate-attr`, `empty-block`: Avoid self-edges, duplicate edges in a link group, repeated keys in a block, and empty `link`/`narrative` blocks.

## Reference Guides

- **Full language spec, modeling conventions & diagnostic fix guide**: read [`references/syntax-and-modeling.md`](references/syntax-and-modeling.md).
- **Custom calendars (`date_mode`) & calculable properties (`calculable_props`) in Rune (`.rn`)**: read [`references/rune-plugins.md`](references/rune-plugins.md).
