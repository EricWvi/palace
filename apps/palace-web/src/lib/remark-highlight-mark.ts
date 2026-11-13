import type { Parent, PhrasingContent } from "mdast";
import type { Extension as FromMarkdownExtension } from "mdast-util-from-markdown";
import type {
  Code,
  Event,
  Extension,
  Resolver,
  State,
  Token,
  TokenizeContext,
  Tokenizer,
} from "micromark-util-types";
import type { Processor } from "unified";

declare module "micromark-util-types" {
  interface TokenTypeMap {
    highlightMark: "highlightMark";
    highlightMarkSequence: "highlightMarkSequence";
    highlightMarkSequenceTemporary: "highlightMarkSequenceTemporary";
    highlightMarkText: "highlightMarkText";
  }
}

// `==text==` in the syntax tree; it renders as `<mark>`.
interface HighlightMark extends Parent {
  type: "highlightMark";
  children: PhrasingContent[];
}

declare module "mdast" {
  interface PhrasingContentMap {
    highlightMark: HighlightMark;
  }
  interface RootContentMap {
    highlightMark: HighlightMark;
  }
}

// Where remark-parse reads tree extensions from; it declares this, but nothing here loads its types.
declare module "unified" {
  interface Data {
    fromMarkdownExtensions?: FromMarkdownExtension[];
  }
}

const EQUALS = 61;
const SPACE = 32;

// micromark's virtual codes (tab, virtual space, line endings) are negative; null is the end.
function isWhitespace(code: Code) {
  return (
    code === null ||
    code < 0 ||
    code === SPACE ||
    /\s/.test(String.fromCharCode(code))
  );
}

// Pairs each closing `==` with the nearest opener before it, the way GFM pairs `~~`, and resolves
// emphasis and the like between them, so `==**加粗**==` marks bold text. Sequences left unpaired
// read as plain `==`.
const resolveAllHighlightMark: Resolver = (events, context) => {
  let index = -1;
  while (++index < events.length) {
    const closer = events[index];
    if (!(
      closer[0] === "enter" &&
      closer[1].type === "highlightMarkSequenceTemporary" &&
      closer[1]._close
    ))
      continue;
    let open = index;
    while (open--) {
      const opener = events[open];
      if (!(
        opener[0] === "exit" &&
        opener[1].type === "highlightMarkSequenceTemporary" &&
        opener[1]._open
      ))
        continue;
      opener[1].type = "highlightMarkSequence";
      closer[1].type = "highlightMarkSequence";
      const mark: Token = {
        type: "highlightMark",
        start: { ...opener[1].start },
        end: { ...closer[1].end },
      };
      const text: Token = {
        type: "highlightMarkText",
        start: { ...opener[1].end },
        end: { ...closer[1].start },
      };
      const replaced: Event[] = [
        ["enter", mark, context],
        ["enter", opener[1], context],
        ["exit", opener[1], context],
        ["enter", text, context],
        ...resolveInsideSpan(events.slice(open + 1, index), context),
        ["exit", text, context],
        ["enter", closer[1], context],
        ["exit", closer[1], context],
        ["exit", mark, context],
      ];
      // From the opener's enter to the closer's exit.
      events.splice(open - 1, index - open + 3, ...replaced);
      index = open + replaced.length - 2;
      break;
    }
  }
  for (const [, token] of events) {
    if (token.type === "highlightMarkSequenceTemporary") token.type = "data";
  }
  return events;
};

// Runs each inline resolver once over the events between a pair, as micromark does for a span.
function resolveInsideSpan(events: Event[], context: TokenizeContext): Event[] {
  const called: Resolver[] = [];
  for (const { resolveAll } of context.parser.constructs.insideSpan.null ??
    []) {
    if (!resolveAll || called.includes(resolveAll)) continue;
    called.push(resolveAll);
    events = resolveAll(events, context);
  }
  return events;
}

// Reads exactly two `=` as a possible opener or closer: `=` and `===` stay text, as does an
// escaped `\==`. Only whitespace decides which side a sequence may close, never punctuation as
// in CommonMark, so `==重点。==后文` marks the way `**` does with remark-cjk-friendly.
const tokenizeHighlightMark: Tokenizer = function (
  this: TokenizeContext,
  effects,
  ok,
  nok,
) {
  const previous = this.previous;
  const events = this.events;
  let size = 0;
  const more: State = (code) => {
    if (code === EQUALS) {
      if (size > 1) return nok(code);
      effects.consume(code);
      size++;
      return more;
    }
    if (size < 2) return nok(code);
    const token = effects.exit("highlightMarkSequenceTemporary");
    token._open = !isWhitespace(code);
    token._close = !isWhitespace(previous);
    return ok(code);
  };
  return (code) => {
    if (
      previous === EQUALS &&
      events[events.length - 1][1].type !== "characterEscape"
    )
      return nok(code);
    effects.enter("highlightMarkSequenceTemporary");
    return more(code);
  };
};

const construct = {
  name: "highlightMark",
  tokenize: tokenizeHighlightMark,
  resolveAll: resolveAllHighlightMark,
};

const syntax: Extension = {
  text: { [EQUALS]: construct },
  insideSpan: { null: [construct] },
  // Lets `*` beside `==` still open and close emphasis, as GFM does for `~`.
  attentionMarkers: { null: [EQUALS] },
};

const tree: FromMarkdownExtension = {
  canContainEols: ["highlightMark"],
  enter: {
    highlightMark(token) {
      this.enter(
        { type: "highlightMark", children: [], data: { hName: "mark" } },
        token,
      );
    },
  },
  exit: {
    highlightMark(token) {
      this.exit(token);
    },
  },
};

// Adds `==text==` highlights, rendered as `<mark>`.
export function remarkHighlightMark(this: Processor) {
  const data = this.data();
  (data.micromarkExtensions ??= []).push(syntax);
  (data.fromMarkdownExtensions ??= []).push(tree);
}
