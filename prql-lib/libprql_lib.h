#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

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
