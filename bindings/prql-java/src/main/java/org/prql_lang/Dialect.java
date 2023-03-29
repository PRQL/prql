package org.prql_lang;

import java.io.Serializable;

/**
 * Dialect supported by PRQL.
 */
public interface Dialect extends Serializable {
    /**
     * Gets prefix, for example: {@code sql} for SQL dialect.
     *
     * @return non-null prefix
     */
    String getPrefix();

    /**
     * Gets key, usually with a prefix, for instance: {@code sql.mysql}.
     *
     * @return non-null key
     */
    String getKey();
}