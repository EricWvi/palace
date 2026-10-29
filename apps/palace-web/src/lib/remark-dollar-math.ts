import type {
  Code,
  Construct,
  Extension,
  State,
  TokenizeContext,
  Tokenizer,
} from "micromark-util-types";
import type { Processor } from "unified";

// The token types remark-math's mdast handlers turn into an `inlineMath` node.
declare module "micromark-util-types" {
  interface TokenTypeMap {
    mathText: "mathText";
    mathTextData: "mathTextData";
    mathTextSequence: "mathTextSequence";
  }
}

// Where remark-parse reads syntax extensions from; it declares this, but nothing here loads its types.
declare module "unified" {
  interface Data {
    micromarkExtensions?: Extension[];
  }
}

const DOLLAR = 36;
const BACKSLASH = 92;
const SPACE = 32;
// micromark's virtual codes: tab, virtual space, and the three line endings.
const HORIZONTAL_TAB = -2;
const VIRTUAL_SPACE = -1;
const CARRIAGE_RETURN = -5;
const LINE_FEED = -4;
const CARRIAGE_RETURN_LINE_FEED = -3;

function isLineEnding(code: Code) {
  return (
    code === CARRIAGE_RETURN ||
    code === LINE_FEED ||
    code === CARRIAGE_RETURN_LINE_FEED
  );
}

function isWhitespace(code: Code) {
  return (
    code === SPACE ||
    code === HORIZONTAL_TAB ||
    code === VIRTUAL_SPACE ||
    isLineEnding(code)
  );
}

function isDigit(code: Code) {
  return code !== null && code >= 48 && code <= 57;
}

// Reads `$x$` as a formula by Pandoc's rules, which prose with dollar amounts needs: the opening
// `$` is followed by a non-space, and the closing `$` follows a non-space and precedes no digit.
// Unlike Pandoc, only the next unescaped `$` may close: in `单个 $：……里 $x$` an opening `$` that
// skipped the invalid closer before `x` would swallow the real formula, so the attempt fails
// there and that `$` stays text. Emits remark-math's own tokens so its mdast handlers apply.
const tokenizeSingleDollarMath: Tokenizer = function (
  this: TokenizeContext,
  effects,
  ok,
  nok,
) {
  let afterWhitespace = false;

  const afterOpen: State = (code) => {
    if (code === null || code === DOLLAR || isWhitespace(code))
      return nok(code);
    effects.enter("mathTextData");
    return data(code);
  };

  const data: State = (code) => {
    if (code === null) return nok(code);
    if (code === DOLLAR) {
      if (afterWhitespace) return nok(code);
      effects.exit("mathTextData");
      effects.enter("mathTextSequence");
      effects.consume(code);
      effects.exit("mathTextSequence");
      effects.exit("mathText");
      return afterClose;
    }
    // The formula is a paragraph's inline content, so a soft line break may fall inside it.
    if (isLineEnding(code)) {
      effects.exit("mathTextData");
      effects.enter("lineEnding");
      effects.consume(code);
      effects.exit("lineEnding");
      effects.enter("mathTextData");
      afterWhitespace = true;
      return data;
    }
    afterWhitespace = isWhitespace(code);
    effects.consume(code);
    return code === BACKSLASH ? escaped : data;
  };

  // `\$` is a dollar sign inside the formula, not its end; TeX keeps the backslash.
  const escaped: State = (code) => {
    if (code !== DOLLAR) return data(code);
    afterWhitespace = false;
    effects.consume(code);
    return data;
  };

  // `$5-$10` has no formula: a closer right before a digit is an amount's sign.
  const afterClose: State = (code) => (isDigit(code) ? nok(code) : ok(code));

  return (code) => {
    effects.enter("mathText");
    effects.enter("mathTextSequence");
    effects.consume(code);
    effects.exit("mathTextSequence");
    return afterOpen;
  };
};

const singleDollarMath: Construct = {
  name: "singleDollarMath",
  tokenize: tokenizeSingleDollarMath,
  // `$$x$$` belongs to remark-math, and `\$` is an escaped sign rather than an opener.
  previous(code) {
    return (
      code !== DOLLAR ||
      this.events[this.events.length - 1][1].type === "characterEscape"
    );
  },
};

const syntax: Extension = { text: { [DOLLAR]: singleDollarMath } };

// Adds `$x$` formulas to remark-math, which must run with `singleDollarTextMath: false`.
export function remarkDollarMath(this: Processor) {
  const data = this.data();
  (data.micromarkExtensions ??= []).push(syntax);
}
