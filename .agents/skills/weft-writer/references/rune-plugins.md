# Weft Rune (`.rn`) Plugin Reference

Weft extends stories with two kinds of sandboxed [Rune](https://rune-rs.github.io/) scripts (`.rn`, embedded `rune` 0.14.x), referenced relative to the `.weft` file:

1. **Custom calendars** (`story.date_mode = "my_calendar.rn"`)
2. **Calculable properties** (`story.calculable_props = { my_prop = "my_prop.rn" }`)

## Official Rune Language Docs

When writing non-trivial logic or hitting unfamiliar Rune syntax, consult the [Rune Book](https://rune-rs.github.io/book/):

- [Variables & types](https://rune-rs.github.io/book/variables.html) (`let`, `const`, integers, arrays `[...]`)
- [Functions](https://rune-rs.github.io/book/functions.html) (`pub fn` exports vs private `fn`)
- [Control flow](https://rune-rs.github.io/book/control_flow.html) (`if` / `else`, `while`, `for`, `return`, `break`)
- [Pattern matching](https://rune-rs.github.io/book/pattern_matching.html) (`match`, handling `()` unit for missing optional fields)
- [Anonymous objects](https://rune-rs.github.io/book/objects.html) (`#{ key: value }` returned by `meta(ctx)` or `calculable_prop(ctx)`)
- [Template literals](https://rune-rs.github.io/book/template_literals.html) (`` `${expr}` `` string interpolation used in `humanizer(ctx)`)

> **Important difference from examples in the Rune Book**: Weft constructs its VM with `Context::with_config(false)` (no I/O). Do **not** copy `println!`, `print!`, or `dbg!` from Rune Book snippets—they will fail to compile in Weft's sandbox.

## Sandbox & Runtime Constraints

- **No I/O**: Host builds `Context::with_config(false)`—there is no `println!`, `dbg!`, filesystem, or network access.
- **Limits**: 1,000,000 instruction budget per entry call; max script file size 1 MB; file extension must be `.rn`.
- **API version**: `ctx.api` is `1`.
- **Localization**: `ctx.language` is `"zh"` or `"en"` (controlled via CLI `--lang zh|en` or system locale). Support both when practical, and test both with `weft check --lang zh` and `weft check --lang en`.
- **Rune gotchas**:
  - JSON `null` deserializes to Rune unit `()`, and returning `()` from `calculable_prop` means "omit this property".
  - String interpolation uses backticks: `` `${year}年${month}月` ``.
  - Anonymous objects use `#{ key: value }`.
  - When calling `panic(message)` conditionally, wrap it in `match` rather than `if` if the surrounding function returns a non-unit value—in Rune, an `if` branch with `panic` can trigger unit-type inference errors and swallow the message.
  - Use explicit floor division / positive modulo helpers when `tick` or offsets can be negative, because `/` and `%` truncate toward zero in Rune (just like Rust).

---

## 1. Custom Calendar Script (`date_mode`)

A calendar `.rn` script MUST export four `pub fn` entry points:

| Function | Signature | Purpose |
| --- | --- | --- |
| `meta(ctx)` | `ctx: #{ api, language }` -> `#{ id, name, description?, components, minimums }` | Calendar metadata & shape |
| `to_tick(values)` | `values: Vec<i64>` (len = `components`) -> `i64` | Convert date components (including unnormalized offset results!) to linear integer tick |
| `from_tick(tick)` | `tick: i64` -> `Vec<i64>` (len = `components`) | Convert linear tick back to normalized date components |
| `humanizer(ctx)` | `ctx: #{ api, values, precision, language }` -> `String` | Format normalized date respecting author's `precision` |

### Critical Invariants Enforced by `weft check`

1. **`meta` validation**:
   - `id` and `name` must be non-empty strings.
   - `components` must be in `1..=64`.
   - `minimums.len()` must equal `components`. `minimums[0]` is informational (the highest component has no lower bound); for `i > 0`, any absolute time in `.weft` with `value < minimums[i]` fails with `time-component-below-minimum`.
2. **Normalization = `from_tick(to_tick(values))`**:
   - When resolving `anchor + offset` or `anchor - offset`, the host adds/subtracts component-wise first, then passes the **unnormalized** array directly to `to_tick(values)`!
   - Therefore `to_tick(values)` MUST gracefully handle out-of-range and negative component values (e.g., month `13`, month `0`, day `-5`, hour `26`), carrying/borrowing them into the linear tick.
3. **Round-trip check (`calendar-round-trip-mismatch`)**:
   - After computing `norm = from_tick(tick)`, the host immediately verifies `to_tick(norm) == tick`. Any mismatch fails compilation.
4. **Precision in `humanizer(ctx)`**:
   - `ctx.precision` is the number of components the author wrote (or `max(base, offset)` for relative times), from `1` up to `components`. Format only up to `precision` (or show time-of-day only when precision covers it or values are non-zero).

### Minimal Calendar Template (Fixed-Length Sci-Fi / Fantasy Calendar)

Example: 3-component calendar `[Year, Month, Day]` with 10 months of 30 days each (300 days/year):

```rune
// 3-component custom calendar: [Year, Month, Day], 10 months x 30 days = 300 days/year.

pub fn meta(ctx) {
    let zh = ctx.language == "zh";
    #{
        id: "imperial-decimal",
        name: if zh { "帝国十进制历" } else { "Imperial Decimal Calendar" },
        description: if zh {
            "每年 10 个月，每月 30 日，共 300 日。"
        } else {
            "10 months of 30 days each (300 days per year)."
        },
        components: 3,
        minimums: [1, 1, 1],
    }
}

const DAYS_PER_MONTH = 30;
const MONTHS_PER_YEAR = 10;
const DAYS_PER_YEAR = 300;

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
    let month = values[1];
    let day = values[2];
    // Handles arbitrary negative/overflowed months and days linearly
    (year - 1) * DAYS_PER_YEAR + (month - 1) * DAYS_PER_MONTH + (day - 1)
}

pub fn from_tick(tick) {
    let year = floor_div(tick, DAYS_PER_YEAR) + 1;
    let rem = pos_mod(tick, DAYS_PER_YEAR);
    let month = rem / DAYS_PER_MONTH + 1;
    let day = rem % DAYS_PER_MONTH + 1;
    [year, month, day]
}

pub fn humanizer(ctx) {
    let v = ctx.values;
    let p = ctx.precision;
    let y = v[0];
    let m = v[1];
    let d = v[2];
    if ctx.language == "zh" {
        if p <= 1 {
            `帝历${y}年`
        } else if p == 2 {
            `帝历${y}年${m}月`
        } else {
            `帝历${y}年${m}月${d}日`
        }
    } else if p <= 1 {
        `Year ${y}`
    } else if p == 2 {
        `Year ${y}, Month ${m}`
    } else {
        `Year ${y}, Month ${m}, Day ${d}`
    }
}
```

For leap-year or variable-month calendars, model `to_tick` / `from_tick` using cycle math (see `examples/lunar.rn` or `crates/weft-core/rune/gregorian.rn` in the weft repo if available).

---

## 2. Calculable Property Script (`calculable_props`)

Register in `story`:

```weft
story {
  title = "..."
  calculable_props = {
    zodiac = "zodiac.rn"
  }
}
```

Enable on an entity:

```weft
entity Alice {
  base_time        = [1996, 8, 12]
  calculable_props = [zodiac]
}
```

### Required Interface

Export `pub fn calculable_prop(ctx)`:

- **`ctx` structure**:
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
      "id": "Alice",
      "base_time": {
        "values": [1996, 8, 12, 0, 0, 0],
        "precision": 3,
        "tick": 63006912000
      },
      "description": "...",
      "belong_to": ["GroupA"],
      "extra_props": { "role": "hero" }
    }
  }
  ```
  - `ctx.entity.base_time` and `ctx.entity.description` are `()` (unit) when absent on the entity!
  - `ctx.entity.extra_props` contains the entity's raw static extra fields (not outputs of other calculable props).
- **Return value**:
  - Return `()` to skip writing the key (recommended when optional input data like `base_time` is missing or lacks precision).
  - Otherwise return any JSON-serializable value: string, int, float, bool, array `[...]`, or object `#{ ... }`. The host stores it in `entity.extra_props[<registered_id>]`.

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
    if bt.precision < 1 {
        return ();
    }

    let year = bt.values[0];
    let animals = if zh {
        ["猴", "鸡", "狗", "猪", "鼠", "牛", "虎", "兔", "龙", "蛇", "马", "羊"]
    } else {
        ["Monkey", "Rooster", "Dog", "Pig", "Rat", "Ox", "Tiger", "Rabbit", "Dragon", "Snake", "Horse", "Goat"]
    };
    let idx = year % 12;
    let idx = if idx < 0 { idx + 12 } else { idx };
    animals[idx]
}
```

## Verification Checklist for `.rn` Plugins

Always verify plugins by running `weft check` WITHOUT `--syntax-only`, exercising both locales and edge cases (missing `base_time`, low precision, negative/overflow time offsets):

```bash
"$WEFT" check path/to/story.weft --deny-warnings --lang zh
"$WEFT" check path/to/story.weft --deny-warnings --lang en
```
