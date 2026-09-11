import { styleTags, tags as t } from "@lezer/highlight";

// The selectors, separate from the `styleTags` call below so that
// `test/test-highlight.js` can check each one resolves to a term in the
// grammar: `styleTags` drops a selector that matches nothing without
// reporting it.
export const prqlHighlightSpec = {
  "CallExpression/Identifier": t.function(t.variableName),
  // Keywords are named terms only because the grammar declares them through
  // `kw<>`; see the note on the literal tokens in `prql.grammar`.
  prql: t.keyword,
  module: t.moduleKeyword,
  let: t.definitionKeyword,
  case: t.controlKeyword,
  in: t.operatorKeyword,
  Annotation: t.annotation,
  Comment: t.lineComment,
  Docblock: t.docString,
  "this that": t.self,
  null: t.null,
  Boolean: t.bool,
  Integer: t.integer,
  Float: t.float,
  DateTime: t.color,
  DeclarationItem: t.propertyName,
  TypeName: t.typeName,
  Escape: t.escape,
  String: t.string,
  FString: t.special(t.string),
  RString: t.special(t.string),
  SString: t.special(t.string),
  TimeUnit: t.unit,
  ArithOp: t.arithmeticOperator,
  CompareOp: t.compareOperator,
  LogicOp: t.logicOperator,
  Equals: t.definitionOperator,
  Parameter: t.processingInstruction,
  VariableName: t.variableName,
  "( )": t.paren,
  "[ ]": t.squareBracket,
  "{ }": t.brace,
  "| ,": t.separator,
};

export const prqlHighlight = styleTags(prqlHighlightSpec);
