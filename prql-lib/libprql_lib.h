#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

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
 * This function assumes zero-terminated strings and sufficiently large output buffers.
 */
int compile(const char *prql_query, const struct Options *options, char *out);

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
 * This function assumes zero-terminated strings and sufficiently large output buffers.
 */
int prql_to_pl(const char *prql_query, char *out);

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
 * This function assumes zero-terminated strings and sufficiently large output buffers.
 */
int pl_to_rq(const char *pl_json, char *out);

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
 * This function assumes zero-terminated strings and sufficiently large output buffers.
 */
int rq_to_sql(const char *rq_json, char *out);
