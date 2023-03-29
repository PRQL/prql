package org.prql_lang;

/**
 * This class represents compilation options supported by prqlc. Besides the
 * ones defined in {@code prql_compiler::Options}, more options like
 * {@code exception} were added for Java binding.
 */
public class CompileOptions {
    /**
     * Compilation options producing compact output.
     */
    public static final CompileOptions COMPACT = new CompileOptions(false, SqlDialect.ANY, false);

    /**
     * Default compilation options.
     */
    public static final CompileOptions DEFAULT = new CompileOptions(true, SqlDialect.ANY, true);

    private final boolean format;
    private final Dialect dialect;
    // in case prql-java is not in sync with the native library
    private final String target;
    // signature comment
    private final boolean comment;

    /**
     * Default constructor.
     *
     * @param format   whether to format the output
     * @param dialect  compilation target, null is treated as empty string
     * @param comment  whether to append signature comment in output
     * @param fallback whether to fall back to the input when failed to compile PRQL
     */
    public CompileOptions(boolean format, Dialect dialect, boolean comment) {
        this.format = format;
        if (dialect == null) {
            this.dialect = SqlDialect.ANY;
        } else {
            this.dialect = dialect;
        }
        this.target = this.dialect.getKey();
        this.comment = comment;
    }

    /**
     * Constructor for custom target.
     *
     * @param format   whether to format the output
     * @param target   custom compilation target, null is treated as empty string
     * @param comment  whether to append signature comment in output
     * @param fallback whether to fall back to the input when failed to compile PRQL
     */
    public CompileOptions(boolean format, String target, boolean comment) {
        this.format = format;
        if (target == null || target.isEmpty()) {
            this.dialect = SqlDialect.ANY;
            this.target = this.dialect.getKey();
        } else {
            this.dialect = SqlDialect.of(target);
            this.target = target;
        }
        this.comment = comment;
    }

    /**
     * Gets target dialect.
     *
     * @return non-null target dialect
     */
    public Dialect getDialect() {
        return dialect;
    }

    /**
     * Gets compilation target.
     *
     * @return non-empty compilation target
     */
    public String getTarget() {
        return target;
    }

    /**
     * Whether to format the output.
     *
     * @return true if format is required; false otherwise
     */
    public boolean requireFormat() {
        return format;
    }

    /**
     * Whether to append signature comments at the end of output.
     *
     * @return true if signature comments is required; false otherwise
     */
    public boolean requireComment() {
        return comment;
    }
}
