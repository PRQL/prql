import { styleTags, tags as t } from "@lezer/highlight";

export const prqlHighlight = styleTags({
  let: t.definitionKeyword,
  case: t.controlKeyword,
  in: t.operatorKeyword,
  Comment: t.lineComment,
  Number: t.number,
  String: t.string,
  F_string: t.special(t.string),
  R_string: t.special(t.string),
  S_string: t.special(t.string),
  "( )": t.paren,
  "[ ]": t.squareBracket,
  "{ }": t.brace,
  "| ,": t.separator,
});
