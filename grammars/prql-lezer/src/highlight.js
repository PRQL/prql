import { styleTags, tags as t } from "@lezer/highlight";

export const prqlHighlight = styleTags({
  "CallExpression/Identifier": t.function(t.variableName),
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
});
