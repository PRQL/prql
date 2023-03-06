#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Location within the source file.
 * Tuples contain:
 * - line number (0-based),
 * - column number within that line (0-based),
 *
 */
typedef struct SourceLocation SourceLocation;

typedef struct Span {
  size_t start;
  size_t end;
} Span;

/**
 * An error message.
 *
 * Calling code is responsible for freeing all memory allocated
 * for fields as well as strings.
 */
typedef struct ErrorMessage {
  /**
   * Machine-readable identifier of the error
   */
  const int8_t *const *code;
  /**
   * Plain text of the error
   */
  const int8_t *reason;
  /**
   * A list of suggestions of how to fix the error
   */
  const int8_t *const *hint;
  /**
   * Character offset of error origin within a source file
   */
  const struct Span *span;
  /**
   * Annotated code, containing cause and hints.
   */
  const int8_t *const *display;
  /**
   * Line and column number of error origin within a source file
   */
  const struct SourceLocation *location;
} ErrorMessage;

typedef struct CompileResult {
  const int8_t *output;
  const struct ErrorMessage *errors;
  size_t errors_len;
  size_t errors_capacity;
} CompileResult;

/**
 * Compilation options
 */
typedef struct Options {
  /**
   * Pass generated SQL string trough a formatter that splits it
   * into multiple lines and prettifies indentation and spacing.
   *
   * Defaults to true.
   */
  bool format;
  /**
   * Target and dialect to compile to.
   *
   * Defaults to `sql.any`, which uses `target` argument from the query header to determine
   * the SQL dialect.
   */
  char *target;
  /**
   * Emits the compiler signature as a comment after generated SQL
   *
   * Defaults to true.
   */
  bool signature_comment;
} Options;

/**
 * Compile a PRQL string into a SQL string.
 *
 * This is a wrapper for: `prql_to_pl`, `pl_to_rq` and `rq_to_sql` without converting to JSON
 * between each of the functions.
 *
 * See `Options` struct for available compilation options.
 *
 * # Safety
 *
 * This function assumes zero-terminated input strings.
 * Calling code is responsible for freeing memory allocated for `CompileResult`
 * by calling `result_destroy`.
 */
struct CompileResult compile(const char *prql_query, const struct Options *options);

/**
 * Build PL AST from a PRQL string. PL in documented in the
 * [prql-compiler Rust crate](https://docs.rs/prql-compiler/latest/prql_compiler/ast/pl).
 *
 * Takes PRQL source buffer and writes PL serialized as JSON to `out` buffer.
 *
 * Returns 0 on success and a negative number -1 on failure.
 *
 * # Safety
 *
 * This function assumes zero-terminated input strings.
 * Calling code is responsible for freeing memory allocated for `CompileResult`
 * by calling `result_destroy`.
 */
struct CompileResult prql_to_pl(const char *prql_query);

/**
 * Finds variable references, validates functions calls, determines frames and converts PL to RQ.
 * PL and RQ are documented in the
 * [prql-compiler Rust crate](https://docs.rs/prql-compiler/latest/prql_compiler/ast).
 *
 * Takes PL serialized as JSON buffer and writes RQ serialized as JSON to `out` buffer.
 *
 * Returns 0 on success and a negative number -1 on failure.
 *
 * # Safety
 *
 * This function assumes zero-terminated input strings.
 * Calling code is responsible for freeing memory allocated for `CompileResult`
 * by calling `result_destroy`.
 */
struct CompileResult pl_to_rq(const char *pl_json);

/**
 * Convert RQ AST into an SQL string. RQ is documented in the
 * [prql-compiler Rust crate](https://docs.rs/prql-compiler/latest/prql_compiler/ast/rq).
 *
 * Takes RQ serialized as JSON buffer and writes SQL source to `out` buffer.
 *
 * Returns 0 on success and a negative number -1 on failure.
 *
 * # Safety
 *
 * This function assumes zero-terminated input strings.
 * Calling code is responsible for freeing memory allocated for `CompileResult`
 * by calling `result_destroy`.
 */
struct CompileResult rq_to_sql(const char *rq_json);

/**
 * Destroy a `CompileResult` once you are done with it.
 */
void result_destroy(struct CompileResult res);
