package org.prql_lang;

// https://github.com/PRQL/prql/blob/main/prql-compiler/src/sql/dialect.rs#L40-L63
public enum SqlDialect implements Dialect {
    ANY("any"),
    ANSI("ansi"),
    BIGQUERY("bigquery"),
    CLICKHOUSE("clickhouse"),
    DUCKDB("duckdb"),
    GENERIC("generic"),
    HIVE("hive"),
    MSSQL("mssql"),
    MARIADB("mysql"),
    MYSQL("mysql"),
    POSTGRESQL("postgres"),
    SQLITE("sqlite"),
    SNOWFLAKE("snowflake");

    static final String PREFIX = "sql.";

    /**
     * Similar as {@link #valueOf(String)} but uses case-insensitive {@code key} for
     * lookup. {@link #ANY} will be returned if the key is not supported.
     *
     * @param key case-insensitive key may or may have {@code sql.} prefix
     * @return non-null dialect
     */
    static SqlDialect of(String key) {
        if (key != null && !key.isEmpty()) {
            if (!key.startsWith(PREFIX)) {
                key = PREFIX + key;
            }

            for (SqlDialect dialect : values()) {
                if (dialect.getKey().equalsIgnoreCase(key)) {
                    return dialect;
                }
            }
        }
        return ANY;
    }

    private final String key;

    /**
     * Default constructor.
     *
     * @param key key without prefix
     */
    private SqlDialect(String key) {
        this.key = PREFIX + key;
    }

    @Override
    public String getPrefix() {
        return PREFIX.substring(0, PREFIX.length() - 1);
    }

    @Override
    public String getKey() {
        return key;
    }
}
