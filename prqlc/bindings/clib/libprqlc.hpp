/*
 * PRQL is a modern language for transforming data — a simple, powerful, pipelined SQL replacement
 *
 * License: Apache-2.0
 * Website: https://prql-lang.org/
 */

/* This file is autogenerated. Do not modify this file manually. */

#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>
#define FFI_SCOPE "PRQL"

namespace prqlc {

/// Compile message kind. Currently only Error is implemented.
enum class MessageKind {
  Error,
  Warning,
  Lint,
};

/// Identifier of a location in source.
/// Contains offsets in terms of chars.
struct Span {
  size_t start;
  size_t end;
};

/// Location within a source file.
struct SourceLocation {
  size_t start_line;
  size_t start_col;
  size_t end_line;
  size_t end_col;
};

/// Compile result message.
///
/// Calling code is responsible for freeing all memory allocated
/// for fields as well as strings.
struct Message {
  /// Message kind. Currently only Error is implemented.
  MessageKind kind;
  /// Machine-readable identifier of the error
  const char *const *code;
  /// Plain text of the error
  const char *reason;
  /// A list of suggestions of how to fix the error
  const char *const *hint;
  /// Character offset of error origin within a source file
  const Span *span;
  /// Annotated code, containing cause and hints.
  const char *const *display;
  /// Line and column number of error origin within a source file
  const SourceLocation *location;
};

/// Result of compilation.
struct CompileResult {
  const char *output;
  const Message *messages;
  size_t messages_len;
};

/// Compilation options
struct Options {
  /// Pass generated SQL string trough a formatter that splits it
  /// into multiple lines and prettifies indentation and spacing.
  ///
  /// Defaults to true.
  bool format;
  /// Target and dialect to compile to.
  ///
  /// Defaults to `sql.any`, which uses `target` argument from the query header to determine
  /// the SQL dialect.
  char *target;
  /// Emits the compiler signature as a comment after generated SQL
  ///
  /// Defaults to true.
  bool signature_comment;
};

extern "C" {

/// Compile a PRQL string into a SQL string.
///
/// This is a wrapper for: `prql_to_pl`, `pl_to_rq` and `rq_to_sql` without converting to JSON
/// between each of the functions.
///
/// See `Options` struct for available compilation options.
///
/// # Safety
///
/// This function assumes zero-terminated input strings.
/// Calling code is responsible for freeing memory allocated for `CompileResult`
/// by calling `result_destroy`.
CompileResult compile(const char *prql_query, const Options *options);

/// Build PL AST from a PRQL string. PL in documented in the
/// [prql-compiler Rust crate](https://docs.rs/prql-compiler/latest/prql_compiler/ir/pl).
///
/// Takes PRQL source buffer and writes PL serialized as JSON to `out` buffer.
///
/// Returns 0 on success and a negative number -1 on failure.
///
/// # Safety
///
/// This function assumes zero-terminated input strings.
/// Calling code is responsible for freeing memory allocated for `CompileResult`
/// by calling `result_destroy`.
CompileResult prql_to_pl(const char *prql_query);

/// Finds variable references, validates functions calls, determines frames and converts PL to RQ.
/// PL and RQ are documented in the
/// [prql-compiler Rust crate](https://docs.rs/prql-compiler/latest/prql_compiler/ast).
///
/// Takes PL serialized as JSON buffer and writes RQ serialized as JSON to `out` buffer.
///
/// Returns 0 on success and a negative number -1 on failure.
///
/// # Safety
///
/// This function assumes zero-terminated input strings.
/// Calling code is responsible for freeing memory allocated for `CompileResult`
/// by calling `result_destroy`.
CompileResult pl_to_rq(const char *pl_json);

/// Convert RQ AST into an SQL string. RQ is documented in the
/// [prql-compiler Rust crate](https://docs.rs/prql-compiler/latest/prql_compiler/ir/rq).
///
/// Takes RQ serialized as JSON buffer and writes SQL source to `out` buffer.
///
/// Returns 0 on success and a negative number -1 on failure.
///
/// # Safety
///
/// This function assumes zero-terminated input strings.
/// Calling code is responsible for freeing memory allocated for `CompileResult`
/// by calling `result_destroy`.
CompileResult rq_to_sql(const char *rq_json, const Options *options);

/// Destroy a `CompileResult` once you are done with it.
///
/// # Safety
///
/// This function expects to be called exactly once after the call of any the functions
/// that return `CompileResult`. No fields should be freed manually.
void result_destroy(CompileResult res);

} // extern "C"

} // namespace prqlc
