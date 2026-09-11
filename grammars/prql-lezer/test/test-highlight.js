// A style tag in `highlight.js` whose selector matches no term in the grammar
// is silently dropped rather than reported, so these tests pin each token to
// the tag it should carry — a selector that stops matching turns up here as a
// failure instead of as unstyled output in a downstream editor.
import { parser } from "../dist/index.js";
import { prqlHighlightSpec } from "../src/highlight.js";
import { highlightTree, tagHighlighter, tags as t } from "@lezer/highlight";
import * as assert from "assert";

// One class per tag, rather than `classHighlighter`'s groupings, so that a
// token styled with the wrong tag of a family is still a failure.
const highlighter = tagHighlighter(
  [
    t.keyword,
    t.moduleKeyword,
    t.definitionKeyword,
    t.controlKeyword,
    t.operatorKeyword,
    t.paren,
    t.squareBracket,
    t.brace,
    t.separator,
  ].map((tag) => ({ tag, class: tag.toString() })),
);

// The tag each styled token carries, as `[text, tag]` pairs. Unstyled tokens
// are absent, which is what an unmatched style tag produces.
function tagged(source) {
  const styled = [];
  highlightTree(parser.parse(source), highlighter, (from, to, classes) =>
    styled.push([source.slice(from, to), classes]),
  );
  return styled;
}

describe("highlight", () => {
  it("tags the keywords the grammar names through `kw<>`", () => {
    assert.deepStrictEqual(tagged("prql target:sql.duckdb\nfrom x\n"), [
      ["prql", t.keyword.toString()],
    ]);
    assert.deepStrictEqual(tagged("module m {\nfrom x\n}\n"), [
      ["module", t.moduleKeyword.toString()],
      ["{", t.brace.toString()],
      ["}", t.brace.toString()],
    ]);
    assert.deepStrictEqual(tagged("let a = (from x)\n"), [
      ["let", t.definitionKeyword.toString()],
      ["(", t.paren.toString()],
      [")", t.paren.toString()],
    ]);
    assert.deepStrictEqual(tagged("from x\nderive b = case {a => 1}\n"), [
      ["case", t.controlKeyword.toString()],
      ["{", t.brace.toString()],
      ["}", t.brace.toString()],
    ]);
    assert.deepStrictEqual(tagged("from x\nfilter a in b\n"), [
      ["in", t.operatorKeyword.toString()],
    ]);
  });

  it("tags brackets, braces and separators", () => {
    assert.deepStrictEqual(tagged("from x\nselect {a, b}\n"), [
      ["{", t.brace.toString()],
      [",", t.separator.toString()],
      ["}", t.brace.toString()],
    ]);
    assert.deepStrictEqual(tagged("from x\nderive c = [1]\n"), [
      ["[", t.squareBracket.toString()],
      ["]", t.squareBracket.toString()],
    ]);
    assert.deepStrictEqual(tagged("from x | select a\n"), [
      ["|", t.separator.toString()],
    ]);
  });

  // The snippet cases above pin the tag each token carries, but only for the
  // tokens they contain; this one covers every selector, including any added
  // later for a token no snippet happens to use.
  it("has no selector that matches no term in the grammar", () => {
    const terms = new Set(parser.nodeSet.types.map((type) => type.name));
    // A key holds space-separated selectors, each a `/`-separated path, with
    // `...` standing for any intervening nodes and a trailing `!` or `*`
    // limiting how far down the tag applies.
    const selectorTerms = Object.keys(prqlHighlightSpec)
      .flatMap((key) => key.split(" "))
      .flatMap((selector) => selector.split("/"))
      .map((part) => part.replace(/[!*]$/, ""))
      .filter((part) => part !== "" && part !== "...");

    assert.deepStrictEqual(
      selectorTerms.filter((term) => !terms.has(term)),
      [],
    );
  });
});
