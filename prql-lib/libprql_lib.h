#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct CompileOptions {
  /**
   * Pass generated SQL string trough a formatter that splits it
   * into multiple lines and prettifies indentation and spacing.
   *
   * Defaults to true.
   */
  bool format;
  /**
   * Target and dialect to compile to.
   */
  const char *target;
  /**
   * Emits the compiler signature as a comment after generated SQL
   *
   * Defaults to true.
   */
  bool signature_comment;
} CompileOptions;

/**
 * # Safety
 *
 * This function is inherently unsafe because it is using C ABI.
 */
const char *compile(const char *query, struct CompileOptions options);

/**
 * # Safety
 *
 * This function is inherently unsafe because it is using C ABI.
 */
int to_sql(const char *query, char *out);

/**
 * # Safety
 *
 * This function is inherently unsafe because it using C ABI.
 */
int to_json(const char *query, char *out);
