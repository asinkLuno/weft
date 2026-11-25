import {
  type Completion,
  type CompletionContext,
  type CompletionResult,
  snippetCompletion,
} from "@codemirror/autocomplete";
import type { Lang } from "@/i18n";
import {
  ATOMS,
  type AttrBlockKind,
  BARE_IDENT_RE,
  BLOCK_FIELDS,
  type BlockKind,
  findFieldSpec,
  IDENT_CONT_RE,
  IDENT_PREFIX_RE,
  IDENT_START_RE,
  IDENT_VALID_RE,
  NAMED_DECL_KEYWORDS,
} from "./weft-schema";

// ---- 词法扫描（与 Rust src-tauri/src/weft/parser.rs 对齐） ----

type TokKind =
  | "ident"
  | "str"
  | "num"
  | "dash2"
  | "plus"
  | "minus"
  | "eq"
  | "dot"
  | "comma"
  | "semi"
  | "lbrace"
  | "rbrace"
  | "lbracket"
  | "rbracket";

type Tok = {
  kind: TokKind;
  value: string;
  from: number;
  to: number;
  line: number;
};

type CursorString = { from: number; to: number; multiline: boolean };

type TokenizeResult = {
  tokens: Tok[];
  cursorLine: number;
  cursorInComment: boolean;
  cursorInString: CursorString | null;
};

const SINGLE_CHAR_TOKS: Record<string, TokKind> = {
  "+": "plus",
  "-": "minus",
  "=": "eq",
  ".": "dot",
  ",": "comma",
  ";": "semi",
  "{": "lbrace",
  "}": "rbrace",
  "[": "lbracket",
  "]": "rbracket",
};

function tokenizeWeft(src: string, cursorPos: number): TokenizeResult {
  const tokens: Tok[] = [];
  let i = 0;
  let line = 0;
  let cursorLine = 0;
  let cursorInComment = false;
  let cursorInString: CursorString | null = null;
  const n = src.length;

  const advanceLine = () => {
    if (i < cursorPos) cursorLine += 1;
    line += 1;
  };

  while (i < n) {
    const ch = src[i];
    if (ch === "\n") {
      advanceLine();
      i += 1;
      continue;
    }
    if (ch === " " || ch === "\t" || ch === "\r") {
      i += 1;
      continue;
    }

    // 单行注释 # 或 //
    if (ch === "#" || (ch === "/" && src[i + 1] === "/")) {
      const start = i;
      while (i < n && src[i] !== "\n") i += 1;
      if (cursorPos > start && cursorPos <= i) cursorInComment = true;
      continue;
    }

    // 块注释 /* ... */
    if (ch === "/" && src[i + 1] === "*") {
      const start = i;
      i += 2;
      let closed = false;
      while (i < n) {
        if (src[i] === "*" && src[i + 1] === "/") {
          i += 2;
          closed = true;
          break;
        }
        if (src[i] === "\n") advanceLine();
        i += 1;
      }
      if (cursorPos > start && (closed ? cursorPos < i : cursorPos <= i)) {
        cursorInComment = true;
      }
      continue;
    }

    // 三引号多行字符串 """..."""
    if (ch === '"' && src[i + 1] === '"' && src[i + 2] === '"') {
      const start = i;
      const startLine = line;
      i += 3;
      let val = "";
      let closed = false;
      while (i < n) {
        if (src[i] === '"' && src[i + 1] === '"' && src[i + 2] === '"') {
          i += 3;
          closed = true;
          break;
        }
        if (src[i] === "\\" && i + 1 < n) {
          val += src[i + 1];
          if (src[i + 1] === "\n") advanceLine();
          i += 2;
        } else {
          if (src[i] === "\n") advanceLine();
          val += src[i];
          i += 1;
        }
      }
      if (
        cursorPos >= start + 3 &&
        (closed ? cursorPos <= i - 3 : cursorPos <= i)
      ) {
        cursorInString = { from: start, to: i, multiline: true };
      }
      tokens.push({
        kind: "str",
        value: val.trim(),
        from: start,
        to: i,
        line: startLine,
      });
      continue;
    }

    // 单行字符串 "..."
    if (ch === '"') {
      const start = i;
      const startLine = line;
      i += 1;
      let val = "";
      let closed = false;
      while (i < n && src[i] !== "\n" && src[i] !== "\r") {
        if (src[i] === "\\" && i + 1 < n) {
          val += src[i + 1];
          i += 2;
        } else if (src[i] === '"') {
          i += 1;
          closed = true;
          break;
        } else {
          val += src[i];
          i += 1;
        }
      }
      if (cursorPos > start && (closed ? cursorPos < i : cursorPos <= i)) {
        cursorInString = { from: start, to: i, multiline: false };
      }
      tokens.push({
        kind: "str",
        value: val,
        from: start,
        to: i,
        line: startLine,
      });
      continue;
    }

    // 双短横 --
    if (ch === "-" && src[i + 1] === "-") {
      tokens.push({ kind: "dash2", value: "--", from: i, to: i + 2, line });
      i += 2;
      continue;
    }

    // 单字符符号
    const singleKind = SINGLE_CHAR_TOKS[ch];
    if (singleKind) {
      tokens.push({ kind: singleKind, value: ch, from: i, to: i + 1, line });
      i += 1;
      continue;
    }

    // 数字
    if (ch >= "0" && ch <= "9") {
      const start = i;
      while (i < n && src[i] >= "0" && src[i] <= "9") i += 1;
      if (src[i] === "." && src[i + 1] >= "0" && src[i + 1] <= "9") {
        i += 1;
        while (i < n && src[i] >= "0" && src[i] <= "9") i += 1;
      }
      tokens.push({
        kind: "num",
        value: src.slice(start, i),
        from: start,
        to: i,
        line,
      });
      continue;
    }

    // 标识符（支持 Unicode 与中点 ·）
    const cp = src.codePointAt(i);
    const charStr = cp !== undefined ? String.fromCodePoint(cp) : ch;
    if (IDENT_START_RE.test(charStr)) {
      const start = i;
      i += charStr.length;
      while (i < n) {
        const nextCp = src.codePointAt(i);
        const nextStr =
          nextCp !== undefined ? String.fromCodePoint(nextCp) : "";
        if (!nextStr || !IDENT_CONT_RE.test(nextStr)) break;
        i += nextStr.length;
      }
      tokens.push({
        kind: "ident",
        value: src.slice(start, i),
        from: start,
        to: i,
        line,
      });
      continue;
    }

    i += charStr.length;
  }

  return { tokens, cursorLine, cursorInComment, cursorInString };
}

function isNameTok(tok: Tok | undefined): tok is Tok {
  return tok !== undefined && (tok.kind === "ident" || tok.kind === "str");
}

function escapeQuotedContent(value: string): string {
  return value.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function formatWeftName(name: string): string {
  return BARE_IDENT_RE.test(name) && !ATOMS.has(name)
    ? name
    : JSON.stringify(name);
}

// 引号内统一消耗到右引号末尾（见 createWeftCompletionSource 的 `to` 计算），因此始终补全闭合引号
function applyName(
  name: string,
  inString: CursorString | null,
  suffix = "",
): string {
  if (inString) {
    return `${escapeQuotedContent(name)}"${suffix}`;
  }
  return `${formatWeftName(name)}${suffix}`;
}

// ---- 单遍扫描：同时提取全文件符号表与光标处上下文 ----

type EntitySymbol = {
  name: string;
  hasBaseTime: boolean;
  description?: string;
};
type EventSymbol = { name: string; hasEnd: boolean; description?: string };

type WeftSymbols = {
  hasStory: boolean;
  entities: Map<string, EntitySymbol>;
  events: Map<string, EventSymbol>;
  timeAnchors: Set<string>;
  calculableProps: Set<string>;
  relations: Set<string>;
};

type CursorContext =
  | { kind: "none" }
  | { kind: "top" }
  | { kind: "member"; base: string }
  | { kind: "attrKey"; block: AttrBlockKind; existingKeys: Set<string> }
  | { kind: "entityRef"; exclude: Set<string> }
  | { kind: "eventRef"; exclude: Set<string> }
  | { kind: "calcPropRef"; exclude: Set<string> }
  | { kind: "relationRef" }
  | {
      kind: "timeExpr";
      selfEvent: string | null;
      field: "base_time" | "start" | "end" | "anchor";
    };

function isBeforeCursor(tok: Tok, pos: number): boolean {
  if (tok.from >= pos) return false;
  if (tok.to >= pos && (tok.kind === "ident" || tok.kind === "str")) {
    return false;
  }
  return true;
}

type CursorSnapshot = {
  braceDepth: number;
  bracketDepth: number;
  blockKind: BlockKind | null;
  blockName: string | null;
  activeAttr: string | null;
  activeAttrLine: number;
  topTimeAssignLine: number;
  inNarrativeList: boolean;
  existingKeys: Set<string>;
  listItems: Set<string>;
  last: Tok | undefined;
  prev: Tok | undefined;
  dashCountOnCursorLine: number;
};

function scanDocument(
  tokens: Tok[],
  pos: number,
  cursorLine: number,
  cursorInString: CursorString | null,
): { symbols: WeftSymbols; ctx: CursorContext } {
  const symbols: WeftSymbols = {
    hasStory: false,
    entities: new Map(),
    events: new Map(),
    timeAnchors: new Set(),
    calculableProps: new Set(),
    relations: new Set(),
  };

  let braceDepth = 0;
  let bracketDepth = 0;
  let blockKind: BlockKind | null = null;
  let blockName: string | null = null;
  let inStoryCalcProps = false;
  let activeAttr: string | null = null;
  let activeAttrLine = -1;
  let topTimeAssignLine = -1;
  let inNarrativeList = false;
  const existingKeys = new Set<string>();
  const listItems = new Set<string>();

  let last: Tok | undefined;
  let prev: Tok | undefined;
  let dashCountOnCursorLine = 0;
  let snapshot: CursorSnapshot | null = null;

  const captureSnapshot = (): CursorSnapshot => {
    if (snapshot !== null) return snapshot;
    snapshot = {
      braceDepth,
      bracketDepth,
      blockKind,
      blockName,
      activeAttr,
      activeAttrLine,
      topTimeAssignLine,
      inNarrativeList,
      existingKeys: new Set(existingKeys),
      listItems: new Set(listItems),
      last,
      prev,
      dashCountOnCursorLine,
    };
    return snapshot;
  };

  const consume = (tok: Tok) => {
    if (snapshot !== null) return;
    if (!isBeforeCursor(tok, pos)) {
      captureSnapshot();
      return;
    }
    prev = last;
    last = tok;
    if (tok.line === cursorLine && tok.kind === "dash2") {
      dashCountOnCursorLine += 1;
    }
  };

  for (let i = 0; i < tokens.length; i++) {
    const tok = tokens[i];
    consume(tok);

    if (braceDepth === 0 && bracketDepth === 0 && tok.kind === "ident") {
      if (tok.value === "story" && tokens[i + 1]?.kind === "lbrace") {
        consume(tokens[i + 1]);
        symbols.hasStory = true;
        blockKind = "story";
        blockName = null;
        existingKeys.clear();
        activeAttr = null;
        braceDepth = 1;
        i += 1;
        continue;
      }
      if (
        tok.value === "time" &&
        isNameTok(tokens[i + 1]) &&
        tokens[i + 2]?.kind === "eq"
      ) {
        symbols.timeAnchors.add(tokens[i + 1].value);
        consume(tokens[i + 1]);
        consume(tokens[i + 2]);
        topTimeAssignLine = tokens[i + 2].line;
        i += 2;
        continue;
      }
      if (
        tok.value === "narrative" &&
        isNameTok(tokens[i + 1]) &&
        tokens[i + 2]?.kind === "eq" &&
        tokens[i + 3]?.kind === "lbracket"
      ) {
        consume(tokens[i + 1]);
        consume(tokens[i + 2]);
        consume(tokens[i + 3]);
        inNarrativeList = true;
        listItems.clear();
        bracketDepth = 1;
        i += 3;
        continue;
      }
      if (
        (tok.value === "entity" ||
          tok.value === "event" ||
          tok.value === "link") &&
        isNameTok(tokens[i + 1])
      ) {
        const name = tokens[i + 1].value;
        if (tok.value === "entity" && !symbols.entities.has(name)) {
          symbols.entities.set(name, { name, hasBaseTime: false });
        } else if (tok.value === "event" && !symbols.events.has(name)) {
          symbols.events.set(name, { name, hasEnd: false });
        }
        if (tokens[i + 2]?.kind === "lbrace") {
          consume(tokens[i + 1]);
          consume(tokens[i + 2]);
          blockKind = tok.value;
          blockName = name;
          existingKeys.clear();
          activeAttr = null;
          braceDepth = 1;
          i += 2;
          continue;
        }
      }
    }

    if (tok.kind === "lbrace") {
      braceDepth += 1;
      continue;
    }
    if (tok.kind === "rbrace") {
      if (inStoryCalcProps && braceDepth === 2) inStoryCalcProps = false;
      braceDepth = Math.max(0, braceDepth - 1);
      if (braceDepth === 0) {
        blockKind = null;
        blockName = null;
        activeAttr = null;
        existingKeys.clear();
      }
      continue;
    }
    if (tok.kind === "lbracket") {
      bracketDepth += 1;
      if (bracketDepth === 1) listItems.clear();
      continue;
    }
    if (tok.kind === "rbracket") {
      bracketDepth = Math.max(0, bracketDepth - 1);
      if (bracketDepth === 0) {
        inNarrativeList = false;
        listItems.clear();
      }
      continue;
    }

    if (braceDepth === 1 && bracketDepth === 0) {
      if (blockKind === "link") {
        if (
          tok.kind === "dash2" &&
          isNameTok(tokens[i + 1]) &&
          tokens[i + 2]?.kind === "dash2"
        ) {
          symbols.relations.add(tokens[i + 1].value);
        }
        continue;
      }

      if (isNameTok(tok) && tokens[i + 1]?.kind === "eq") {
        consume(tokens[i + 1]);
        existingKeys.add(tok.value);
        activeAttr = tok.value;
        activeAttrLine = tokens[i + 1].line;
        if (
          blockKind === "story" &&
          tok.value === "calculable_props" &&
          tokens[i + 2]?.kind === "lbrace"
        ) {
          consume(tokens[i + 2]);
          inStoryCalcProps = true;
          braceDepth = 2;
          i += 2;
          continue;
        }
        if (blockKind === "entity" && blockName) {
          const entity = symbols.entities.get(blockName);
          if (entity) {
            if (tok.value === "base_time") entity.hasBaseTime = true;
            else if (
              tok.value === "description" &&
              tokens[i + 2]?.kind === "str"
            ) {
              entity.description = tokens[i + 2].value;
            }
          }
        } else if (blockKind === "event" && blockName) {
          const ev = symbols.events.get(blockName);
          if (ev) {
            if (tok.value === "end") ev.hasEnd = true;
            else if (
              tok.value === "description" &&
              tokens[i + 2]?.kind === "str"
            ) {
              ev.description = tokens[i + 2].value;
            }
          }
        }
        i += 1;
        continue;
      }
    }

    if (inStoryCalcProps && braceDepth === 2) {
      if (isNameTok(tok) && tokens[i + 1]?.kind === "eq") {
        symbols.calculableProps.add(tok.value);
      }
      continue;
    }

    if (snapshot === null && bracketDepth === 1 && isNameTok(tok)) {
      listItems.add(tok.value);
    }
  }

  const finalSnapshot = captureSnapshot();
  return {
    symbols,
    ctx: resolveCursorContext(finalSnapshot, cursorLine, cursorInString),
  };
}

function resolveCursorContext(
  s: CursorSnapshot,
  cursorLine: number,
  cursorInString: CursorString | null,
): CursorContext {
  if (cursorInString?.multiline) return { kind: "none" };

  const memberContext = (): CursorContext | null =>
    s.last?.kind === "dot" && isNameTok(s.prev)
      ? { kind: "member", base: s.prev.value }
      : null;

  // 顶层 narrative 列表：`narrative X = [...]`
  if (s.inNarrativeList && s.bracketDepth === 1) {
    return { kind: "eventRef", exclude: s.listItems };
  }

  // 顶层时间锚点：`time anchor = ...`
  if (
    s.braceDepth === 0 &&
    s.bracketDepth === 0 &&
    s.topTimeAssignLine === cursorLine
  ) {
    return (
      memberContext() ?? { kind: "timeExpr", selfEvent: null, field: "anchor" }
    );
  }

  // 顶层空白/关键字位置
  if (s.braceDepth === 0) {
    if (cursorInString) return { kind: "none" };
    if (s.last?.kind === "ident" && NAMED_DECL_KEYWORDS.has(s.last.value)) {
      return { kind: "none" };
    }
    return { kind: "top" };
  }

  // 嵌套对象内（如 story.calculable_props）不做通用补全
  if (s.braceDepth > 1 || !s.blockKind) return { kind: "none" };

  // link 块：按当前行光标前 `--` 个数区分 左实体 / 关系 / 右实体
  if (s.blockKind === "link") {
    if (s.dashCountOnCursorLine === 1) return { kind: "relationRef" };
    if (s.dashCountOnCursorLine === 0 || s.dashCountOnCursorLine === 2) {
      return { kind: "entityRef", exclude: new Set() };
    }
    return { kind: "none" };
  }

  const fieldSpec = s.activeAttr
    ? findFieldSpec(s.blockKind, s.activeAttr)
    : undefined;

  // 列表 `[...]` 内部
  if (s.bracketDepth === 1 && fieldSpec) {
    if (fieldSpec.valueKind === "entityList") {
      const exclude = new Set(s.listItems);
      if (s.activeAttr === "belong_to" && s.blockName) exclude.add(s.blockName);
      return { kind: "entityRef", exclude };
    }
    if (fieldSpec.valueKind === "calcPropList") {
      return { kind: "calcPropRef", exclude: s.listItems };
    }
    return { kind: "none" };
  }

  // 属性值位置（`attr = ...` 或 `attr = a + ...`）
  const onAttrValue =
    s.activeAttr !== null &&
    (s.activeAttrLine === cursorLine ||
      s.last?.kind === "eq" ||
      s.last?.kind === "plus" ||
      s.last?.kind === "minus");

  if (onAttrValue && s.bracketDepth === 0) {
    if (fieldSpec?.valueKind === "time") {
      return (
        memberContext() ?? {
          kind: "timeExpr",
          selfEvent: s.blockKind === "event" ? s.blockName : null,
          field: fieldSpec.name as "base_time" | "start" | "end",
        }
      );
    }
    return { kind: "none" };
  }

  if (cursorInString) return { kind: "none" };
  return { kind: "attrKey", block: s.blockKind, existingKeys: s.existingKeys };
}

// ---- 候选项构建 ----

const COPY = {
  zh: {
    story: "故事元信息块（必填）",
    time: "独立命名时间锚点",
    entity: "实体块（人物/团体/地点/道具）",
    link: "实体关系组块",
    event: "事件块",
    narrative: "叙事线声明",
    entityType: "实体",
    eventType: "事件",
    anchorType: "时间锚点",
    propType: "可计算属性",
    relationType: "已有关系",
    eventStart: "事件开始时间",
    eventEnd: "事件结束时间",
    entityBaseTime: "实体基准时间",
    selfStart: "当前事件开始时间",
    selfStartOffset: "在当前事件开始时间上偏移",
    snippets: {
      story: 'story {\n  title  = "#{标题}"\n  author = "#{作者}"\n}',
      storyShort: 'story {\n  title = "#{标题}"\n}',
      entity: 'entity #{名称} {\n  description = "#{}"\n}',
      event:
        "event #{1:事件名} {\n  start    = [#{2:1991}, #{3:9}, #{4:1}]\n  entities = [#{0}]\n}",
      link: 'link #{关系组} {\n  #{实体A} -- "#{关系}" -- #{实体B}\n}',
      narrative: "narrative #{叙事线} = [#{}]",
      time: "time #{1:锚点名} = [#{2:1991}, #{3:9}, #{4:1}]",
    },
  },
  en: {
    story: "Story metadata block (required)",
    time: "Named time anchor",
    entity: "Entity block (person/group/place/item)",
    link: "Grouped entity links",
    event: "Event block",
    narrative: "Narrative declaration",
    entityType: "entity",
    eventType: "event",
    anchorType: "time anchor",
    propType: "calculable prop",
    relationType: "relation",
    eventStart: "event start time",
    eventEnd: "event end time",
    entityBaseTime: "entity base_time",
    selfStart: "current event start time",
    selfStartOffset: "offset from current start time",
    snippets: {
      story: 'story {\n  title  = "#{Title}"\n  author = "#{Author}"\n}',
      storyShort: 'story {\n  title = "#{Title}"\n}',
      entity: 'entity #{name} {\n  description = "#{}"\n}',
      event:
        "event #{1:event_name} {\n  start    = [#{2:1991}, #{3:9}, #{4:1}]\n  entities = [#{0}]\n}",
      link: 'link #{group} {\n  #{entity_a} -- "#{relation}" -- #{entity_b}\n}',
      narrative: "narrative #{name} = [#{}]",
      time: "time #{1:anchor_name} = [#{2:1991}, #{3:9}, #{4:1}]",
    },
  },
} as const;

function topCompletions(symbols: WeftSymbols, lang: Lang): Completion[] {
  const t = COPY[lang];
  const s = t.snippets;
  return [
    snippetCompletion(symbols.hasStory ? s.storyShort : s.story, {
      label: "story",
      type: "keyword",
      detail: t.story,
      boost: symbols.hasStory ? -2 : 10,
    }),
    snippetCompletion(s.entity, {
      label: "entity",
      type: "keyword",
      detail: t.entity,
      boost: 3,
    }),
    snippetCompletion(s.event, {
      label: "event",
      type: "keyword",
      detail: t.event,
      boost: 2,
    }),
    snippetCompletion(s.link, {
      label: "link",
      type: "keyword",
      detail: t.link,
      boost: 1,
    }),
    snippetCompletion(s.narrative, {
      label: "narrative",
      type: "keyword",
      detail: t.narrative,
      boost: 1,
    }),
    snippetCompletion(s.time, {
      label: "time",
      type: "keyword",
      detail: t.time,
    }),
  ];
}

function attrKeyCompletions(
  block: AttrBlockKind,
  existing: Set<string>,
  lang: Lang,
): Completion[] {
  return BLOCK_FIELDS[block].map((spec) =>
    snippetCompletion(spec.template, {
      label: spec.name,
      type: "property",
      detail: spec.shortDesc[lang],
      boost: existing.has(spec.name) ? -5 : 2,
    }),
  );
}

function memberCompletions(
  base: string,
  symbols: WeftSymbols,
  lang: Lang,
): Completion[] {
  const t = COPY[lang];
  const ev = symbols.events.get(base);
  if (ev) {
    return [
      { label: "start", type: "property", detail: t.eventStart, boost: 2 },
      {
        label: "end",
        type: "property",
        detail: t.eventEnd,
        boost: ev.hasEnd ? 1 : -1,
      },
    ];
  }
  if (symbols.entities.has(base)) {
    return [
      {
        label: "base_time",
        type: "property",
        detail: t.entityBaseTime,
        boost: 2,
      },
    ];
  }
  return [
    { label: "start", type: "property", detail: t.eventStart },
    { label: "end", type: "property", detail: t.eventEnd },
    { label: "base_time", type: "property", detail: t.entityBaseTime },
  ];
}

function timeExprCompletions(
  selfEvent: string | null,
  field: "base_time" | "start" | "end" | "anchor",
  symbols: WeftSymbols,
  lang: Lang,
  inString: CursorString | null,
): Completion[] {
  const t = COPY[lang];
  const out: Completion[] = [];

  if (!inString && field === "end" && selfEvent) {
    out.push(
      snippetCompletion("start + [0, 0, 0, #{1:2}]", {
        label: "start + [...]",
        type: "keyword",
        detail: t.selfStartOffset,
        boost: 10,
      }),
      { label: "start", type: "keyword", detail: t.selfStart, boost: 8 },
    );
  }

  for (const anchor of symbols.timeAnchors) {
    out.push({
      label: anchor,
      apply: applyName(anchor, inString),
      type: "constant",
      detail: t.anchorType,
      boost: 4,
    });
  }

  for (const ev of symbols.events.values()) {
    if (field === "start" && ev.name === selfEvent) continue;
    out.push({
      label: `${ev.name}.start`,
      apply: applyName(ev.name, inString, ".start"),
      type: "variable",
      detail: t.eventStart,
      info: ev.description,
      boost: 2,
    });
    if (ev.hasEnd && !(field === "end" && ev.name === selfEvent)) {
      out.push({
        label: `${ev.name}.end`,
        apply: applyName(ev.name, inString, ".end"),
        type: "variable",
        detail: t.eventEnd,
        info: ev.description,
        boost: 1,
      });
    }
  }

  for (const ent of symbols.entities.values()) {
    if (!ent.hasBaseTime) continue;
    out.push({
      label: `${ent.name}.base_time`,
      apply: applyName(ent.name, inString, ".base_time"),
      type: "variable",
      detail: t.entityBaseTime,
      info: ent.description,
      boost: 1,
    });
  }

  return out;
}

// ---- 对外 CodeMirror CompletionSource 工厂 ----

export function createWeftCompletionSource(getLang: () => Lang) {
  return (context: CompletionContext): CompletionResult | null => {
    const src = context.state.doc.toString();
    const pos = context.pos;
    const { tokens, cursorLine, cursorInComment, cursorInString } =
      tokenizeWeft(src, pos);

    if (cursorInComment) return null;

    const { symbols, ctx } = scanDocument(
      tokens,
      pos,
      cursorLine,
      cursorInString,
    );
    if (ctx.kind === "none") return null;

    const wordMatch = cursorInString
      ? { from: cursorInString.from + 1, to: pos }
      : context.matchBefore(IDENT_PREFIX_RE);

    const from = wordMatch ? wordMatch.from : pos;
    const hasPrefix = wordMatch !== null && wordMatch.to > wordMatch.from;

    if (
      !context.explicit &&
      ctx.kind !== "member" &&
      !cursorInString &&
      !hasPrefix
    ) {
      return null;
    }

    const lang = getLang();
    const t = COPY[lang];
    let options: Completion[] = [];

    switch (ctx.kind) {
      case "top":
        options = topCompletions(symbols, lang);
        break;
      case "member":
        options = memberCompletions(ctx.base, symbols, lang);
        break;
      case "attrKey":
        options = attrKeyCompletions(ctx.block, ctx.existingKeys, lang);
        break;
      case "entityRef":
        options = Array.from(symbols.entities.values())
          .filter((ent) => !ctx.exclude.has(ent.name))
          .map((ent) => ({
            label: ent.name,
            apply: applyName(ent.name, cursorInString),
            type: "class",
            detail: t.entityType,
            info: ent.description,
          }));
        break;
      case "eventRef":
        options = Array.from(symbols.events.values())
          .filter((ev) => !ctx.exclude.has(ev.name))
          .map((ev) => ({
            label: ev.name,
            apply: applyName(ev.name, cursorInString),
            type: "function",
            detail: t.eventType,
            info: ev.description,
          }));
        break;
      case "calcPropRef":
        options = Array.from(symbols.calculableProps)
          .filter((prop) => !ctx.exclude.has(prop))
          .map((prop) => ({
            label: prop,
            apply: applyName(prop, cursorInString),
            type: "constant",
            detail: t.propType,
          }));
        break;
      case "relationRef":
        options = Array.from(symbols.relations).map((rel) => ({
          label: rel,
          apply: cursorInString
            ? `${escapeQuotedContent(rel)}"`
            : JSON.stringify(rel),
          type: "text",
          detail: t.relationType,
        }));
        break;
      case "timeExpr":
        options = timeExprCompletions(
          ctx.selfEvent,
          ctx.field,
          symbols,
          lang,
          cursorInString,
        );
        break;
    }

    if (options.length === 0) return null;

    const hasClosingQuote =
      cursorInString !== null &&
      !cursorInString.multiline &&
      cursorInString.to > pos &&
      src[cursorInString.to - 1] === '"';

    return {
      from,
      to: hasClosingQuote ? cursorInString.to : pos,
      options,
      validFor: cursorInString ? /^[^"\n]*$/ : IDENT_VALID_RE,
    };
  };
}
