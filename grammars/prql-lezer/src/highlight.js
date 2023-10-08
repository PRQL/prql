import { styleTags, tags as t } from "@lezer/highlight";

export const prqlHighlight = styleTags({
  let: t.definitionKeyword,
  case: t.controlKeyword,
  in: t.operatorKeyword,
  Comment: t.lineComment,
  Docblock: t.docString,
  Integer: t.integer,
  Float: t.float,
  String: t.string,
  FString: t.special(t.string),
  RString: t.special(t.string),
  SString: t.special(t.string),
  TimeUnit: t.unit,
  ArithOp: t.arithmeticOperator,
  CompareOp: t.compareOperator,
  LogicOp: t.logicOperator,
  VariableName: t.variableName,
  "( )": t.paren,
  "[ ]": t.squareBracket,
  "{ }": t.brace,
  "| ,": t.separator,
});
