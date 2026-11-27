# WEFT Rune (`.rn`) Plugin Reference

WEFT extends `.weft` documents with two kinds of sandboxed [Rune](https://rune-rs.github.io/) scripts (`.rn`, embedded `rune` 0.14.x), referenced relative to the `.weft` file:

1. **Custom time axes / calendars** (`story.date_mode = "my_axis.rn"`) — define any discrete linear coordinate system: fiscal calendars (`[FY, Q, Week]`), sprint cycles (`[Release, Sprint, Day]`), elapsed experiment/mission time (`[Day, Hour, Minute]`), historical/lunar calendars, or custom world calendars.
2. **Calculable properties** (`story.calculable_props = { my_prop = "my_prop.rn" }`) — compute derived entity metadata from `base_time`, `belong_to`, `extra_props`, and `calendar` metadata (e.g. era/cohort tags, lifecycle phases, tier labels).

## Official Rune Language Docs

When writing non-trivial logic or hitting unfamiliar Rune syntax, consult the [Rune Book](https://rune-rs.github.io/book/):

- [Variables & types](https://rune-rs.github.io/book/variables.html) (`let`, `const`, integers, arrays `[...]`)
- [Functions](https://rune-rs.github.io/book/functions.html) (`pub fn` exports vs private `fn`)
- [Control flow](https://rune-rs.github.io/book/control_flow.html) (`if` / `else`, `while`, `for`, `return`, `break`)
- [Pattern matching](https://rune-rs.github.io/book/pattern_matching.html) (`match`, handling `()` unit for missing optional fields)
- [Anonymous objects](https://rune-rs.github.io/book/objects.html) (`#{ key: value }` returned by `meta(ctx)` or `calculable_prop(ctx)`)
- [Template literals](https://rune-rs.github.io/book/template_literals.html) (`` `${expr}` `` string interpolation used in `humanizer(ctx)`)

> **Important difference from Rune Book snippets**: WEFT constructs its VM with `Context::with_config(false)` (no I/O). Do **not** use `println!`, `print!`, or `dbg!`—they fail to compile in WEFT's sandbox.

## Sandbox & Runtime Constraints

- **No I/O**: No `println!`, `dbg!`, filesystem, or network access.
- **Limits**: 1,000,000 instruction budget per entry call; max script file size 1 MB; file extension must be `.rn`.
- **API version**: `ctx.api` is `1`.
- **Localization**: `ctx.language` is `"zh"` or `"en"` (set via CLI `--lang zh|en` or system locale). Support both when practical, and test both with `weft check --lang zh` and `weft check --lang en`.
- **Rune gotchas**:
  - JSON `null` deserializes to Rune unit `()`, and returning `()` from `calculable_prop` means "omit this property".
  - String interpolation uses backticks: `` `FY${y} Q${q}` ``.
  - Anonymous objects use `#{ key: value }`.
  - Wrap conditional `panic(message)` inside `match` rather than `if` inside functions returning a non-unit value—in Rune, an `if` branch with `panic` can trigger unit-type inference errors and hide the message.
  - Use explicit floor division / positive modulo helpers when `tick` or offsets can be negative, because `/` and `%` truncate toward zero in Rune (as in Rust).

______________________________________________________________________

## 1. Custom Calendar / Time-Axis Script (`date_mode`)

A `date_mode` `.rn` script MUST export four `pub fn` entry points:

| Function                            | Signature                                                                                                   | Purpose                                                                                                                       |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `meta(ctx)`                         | `ctx: #{ api, language }` -> `#{ id, name, description?, components, minimums }`                            | Axis metadata, arity, and per-component minimums                                                                              |
| `to_tick(values)`                   | `values: Vec<i64>` (len = `components`) -> `i64`                                                            | Map component tuple (including unnormalized offset sums!) to a linear integer tick                                            |
| `from_tick(tick)`                   | `tick: i64` -> `Vec<i64>` (len = `components`)                                                              | Map linear tick back to normalized components                                                                                 |
| `humanizer(ctx)`                    | `ctx: #{ api, values, precision, language }` -> `String`                                                    | Format normalized components respecting `ctx.precision`                                                                       |
| `delta_humanizer(ctx)` *(optional)* | `ctx: #{ api, language, from: #{ values, tick, precision }, to: #{ values, tick, precision } }` -> `String` | Format signed duration `to - from` (used for event-to-entity `base_time` deltas; falls back to signed tick diff when omitted) |

### Critical Invariants Enforced by `weft check`

1. **`meta` validation**:
   - `id` and `name` must be non-empty strings.
   - `components` must be in `1..=64`.
   - `minimums.len()` must equal `components`. `minimums[0]` is informational (component `0` has no lower bound); for `i > 0`, any absolute time in `.weft` with `value < minimums[i]` fails with `time-component-below-minimum`.
2. **Normalization = `from_tick(to_tick(values))`**:
   - For `anchor + offset` / `anchor - offset`, the host adds/subtracts component-wise first, then passes the **unnormalized** array straight to `to_tick(values)`.
   - Therefore `to_tick(values)` MUST handle arbitrary overflowed or negative component values (e.g. quarter `5`, quarter `0`, week `-3`), folding them linearly into the tick.
3. **Round-trip invariant (`calendar-round-trip-mismatch`)**:
   - After `norm = from_tick(tick)`, the host verifies `to_tick(norm) == tick`. Any mismatch fails compilation.
4. **Precision in `humanizer(ctx)`**:
   - `ctx.precision` is the number of components written by the author (or `max(base, offset)` for relative times), in `1..=components`. Format only up to `precision` (or show lower components only when precision covers them or they are non-zero).

### Minimal Time-Axis Template (Fixed-Radix `[Year, Quarter, Week]`)

Example: 3-component fiscal/planning axis `[Year, Quarter, Week]`, 4 quarters × 13 weeks = 52 weeks/year:

```rune
// 3-component time axis: [Year, Quarter, Week], 4 quarters x 13 weeks = 52 weeks/year.

pub fn meta(ctx) {
    let zh = ctx.language == "zh";
    #{
        id: "fiscal-quarter-week",
        name: if zh { "财年季度周历" } else { "Fiscal Quarter-Week Calendar" },
        description: if zh {
            "每年 4 个季度，每季度 13 周，共 52 周。"
        } else {
            "4 quarters of 13 weeks each (52 weeks per year)."
        },
        components: 3,
        minimums: [1, 1, 1],
    }
}

const WEEKS_PER_QUARTER = 13;
const WEEKS_PER_YEAR = 52;

fn floor_div(a, b) {
    let q = a / b;
    if a % b < 0 { q - 1 } else { q }
}

fn pos_mod(a, b) {
    let r = a % b;
    if r < 0 { r + b } else { r }
}

pub fn to_tick(values) {
    let year = values[0];
    let quarter = values[1];
    let week = values[2];
    // Handles arbitrary negative/overflowed quarters and weeks linearly
    (year - 1) * WEEKS_PER_YEAR + (quarter - 1) * WEEKS_PER_QUARTER + (week - 1)
}

pub fn from_tick(tick) {
    let year = floor_div(tick, WEEKS_PER_YEAR) + 1;
    let rem = pos_mod(tick, WEEKS_PER_YEAR);
    let quarter = rem / WEEKS_PER_QUARTER + 1;
    let week = rem % WEEKS_PER_QUARTER + 1;
    [year, quarter, week]
}

pub fn humanizer(ctx) {
    let v = ctx.values;
    let p = ctx.precision;
    let y = v[0];
    let q = v[1];
    let w = v[2];
    if ctx.language == "zh" {
        if p <= 1 {
            `${y}财年`
        } else if p == 2 {
            `${y}财年 Q${q}`
        } else {
            `${y}财年 Q${q} 第${w}周`
        }
    } else if p <= 1 {
        `FY${y}`
    } else if p == 2 {
        `FY${y} Q${q}`
    } else {
        `FY${y} Q${q} W${w}`
    }
}
```

For irregular calendars (leap years, variable month lengths), compute `to_tick` / `from_tick` via cycle math (see `examples/lunar.rn` or `crates/weft-core/rune/gregorian.rn` in the WEFT repo if available).

______________________________________________________________________

## 2. Calculable Property Script (`calculable_props`)

Register in `story`:

```weft
story {
  title            = "平台演进时间轴"
  calculable_props = { cohort = "cohort.rn" }
}
```

Enable on an entity:

```weft
entity "api/v2" {
  base_time        = [2025, 4, 10]
  calculable_props = [cohort]
}
```

### Required Interface

Export `pub fn calculable_prop(ctx)`:

`ctx` structure:

```json
{
  "api": 1,
  "language": "zh",
  "calendar": {
    "id": "gregorian",
    "components": 6,
    "minimums": [0, 1, 1, 0, 0, 0]
  },
  "entity": {
    "id": "api/v2",
    "base_time": {
      "values": [2025, 4, 10, 0, 0, 0],
      "precision": 3,
      "tick": 63911462400
    },
    "description": "...",
    "belong_to": ["平台组"],
    "tags": ["核心服务", "生产环境"],
    "extra_props": { "kind": "service" }
  }
}
```

- `ctx.entity.base_time` and `ctx.entity.description` are `()` (unit) when omitted on the entity.
- `ctx.entity.extra_props` contains the entity's raw static extra fields (not outputs of other calculable props).
- **Return value**:
  - Return `()` to omit the key (recommended when required input like `base_time` is absent or lacks precision).
  - Otherwise return any JSON-serializable value: string, int, float, bool, array `[...]`, or object `#{ ... }`. The host merges it into `entity.extra_props[<registered_id>]`.

### Minimal Calculable Property Template

```rune
fn require_calendar(actual, expected, zh) {
    match actual == expected {
        true => (),
        false => {
            let msg = if zh {
                `该属性需要 ${expected} 历法，当前为 ${actual}`
            } else {
                `this property requires calendar ${expected}, got ${actual}`
            };
            panic(msg)
        }
    }
}

pub fn calculable_prop(ctx) {
    let zh = ctx.language == "zh";
    require_calendar(ctx.calendar.id, "gregorian", zh);

    let bt = match ctx.entity.base_time {
        () => return (),
        bt => bt,
    };
    if bt.precision < 2 {
        return ();
    }

    let year = bt.values[0];
    let month = bt.values[1];
    let quarter = (month - 1) / 3 + 1;
    if zh {
        `${year}年 Q${quarter}`
    } else {
        `${year}-Q${quarter}`
    }
}
```

## Verification Checklist for `.rn` Plugins

Always verify plugins by running `weft check` WITHOUT `--syntax-only`, testing both locales and edge cases (omitted `base_time`, low precision, overflow/negative time offsets):

```bash
"$WEFT" check path/to/doc.weft --deny-warnings --lang zh
"$WEFT" check path/to/doc.weft --deny-warnings --lang en
```
