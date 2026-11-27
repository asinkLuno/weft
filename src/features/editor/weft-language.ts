import { StreamLanguage, type StringStream } from "@codemirror/language";
import {
  ATOMS,
  BUILTIN_PROPS,
  IDENT_CONT_RE,
  IDENT_START_RE,
  NAMED_DECL_KEYWORDS,
  TOP_KEYWORDS,
} from "./weft-schema";

const NUMBER_RE = /^[0-9]+(?:\.[0-9]+)?/;
const OPERATOR_RE = /^[+\-=.]/;
const PUNCTUATION_RE = /^[{}[\],;]/;

function hasFollowingEquals(line: string, startPos: number): boolean {
  for (let i = startPos; i < line.length; i++) {
    const ch = line.charCodeAt(i);
    // space, tab, carriage return
    if (ch === 32 || ch === 9 || ch === 13) continue;
    return ch === 61; // '='
  }
  return false;
}

type WeftParserState = {
  inBlockComment: boolean;
  inMultilineString: boolean;
  expectDeclName: boolean;
};

function consumeMultilineString(
  stream: StringStream,
  state: WeftParserState,
): string {
  while (!stream.eol()) {
    if (stream.match('"""')) {
      state.inMultilineString = false;
      return "string";
    }
    if (stream.peek() === "\\") {
      stream.next();
      stream.next();
    } else {
      stream.next();
    }
  }
  return "string";
}

function consumeBlockComment(
  stream: StringStream,
  state: WeftParserState,
): string {
  while (!stream.eol()) {
    if (stream.match("*/")) {
      state.inBlockComment = false;
      return "comment";
    }
    stream.next();
  }
  return "comment";
}

export const weftLanguage = StreamLanguage.define<WeftParserState>({
  name: "weft",
  startState: () => ({
    inBlockComment: false,
    inMultilineString: false,
    expectDeclName: false,
  }),
  token(stream, state) {
    if (state.inBlockComment) {
      return consumeBlockComment(stream, state);
    }
    if (state.inMultilineString) {
      return consumeMultilineString(stream, state);
    }
    if (stream.eatSpace()) {
      return null;
    }
    if (stream.match("/*")) {
      state.inBlockComment = true;
      return consumeBlockComment(stream, state);
    }
    if (stream.match("#") || stream.match("//")) {
      stream.skipToEnd();
      return "comment";
    }
    if (stream.match('"""')) {
      state.expectDeclName = false;
      state.inMultilineString = true;
      return consumeMultilineString(stream, state);
    }
    if (stream.peek() === '"') {
      state.expectDeclName = false;
      stream.next();
      while (!stream.eol()) {
        const ch = stream.next();
        if (ch === "\\") {
          stream.next();
        } else if (ch === '"') {
          break;
        }
      }
      return "string";
    }
    if (stream.match("--") || stream.match("->") || stream.match("<-")) {
      state.expectDeclName = false;
      return "operator";
    }
    if (stream.match(NUMBER_RE)) {
      state.expectDeclName = false;
      return "number";
    }
    if (stream.match(OPERATOR_RE)) {
      state.expectDeclName = false;
      return "operator";
    }
    if (stream.match(PUNCTUATION_RE)) {
      state.expectDeclName = false;
      return "punctuation";
    }
    if (stream.match(IDENT_START_RE)) {
      while (!stream.eol() && stream.match(IDENT_CONT_RE)) {
        // consume full Unicode identifier
      }
      const word = stream.current();
      if (state.expectDeclName) {
        state.expectDeclName = false;
        return "variableName";
      }
      if (ATOMS.has(word)) return "atom";
      if (TOP_KEYWORDS.has(word)) {
        state.expectDeclName = NAMED_DECL_KEYWORDS.has(word);
        return "keyword";
      }
      if (BUILTIN_PROPS.has(word)) return "propertyName";
      // Look ahead for `=` to highlight custom extra_props keys as propertyName
      if (hasFollowingEquals(stream.string, stream.pos)) {
        return "propertyName";
      }
      return "variableName";
    }
    state.expectDeclName = false;
    stream.next();
    return null;
  },
  languageData: {
    commentTokens: { line: "#" },
    closeBrackets: { brackets: ["[", "{", '"'] },
  },
});
