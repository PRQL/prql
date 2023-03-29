package org.prql_lang.jna;

import com.sun.jna.Library;
import com.sun.jna.Pointer;
import com.sun.jna.Structure;
import com.sun.jna.Structure.FieldOrder;

/**
 * PRQL native library. Please refer to
 * https://github.com/PRQL/prql/blob/main/bindings/prql-lib/libprql_lib.h and
 * maybe https://github.com/PRQL/prql/blob/main/bindings/prql-lib/src/lib.rs for
 * details.
 */
interface PrqlLibrary extends Library {
    // MessageKind enum
    static final int ERROR_MESSAGE = 0; // Error
    static final int WARN_MESSAGE = 1; // Warning
    static final int LINT_MESSAGE = 2; // Lint

    @FieldOrder({ "format", "target", "signature_comment" })
    static class OptionsStruct extends Structure { // NOSONAR
        public static class ByValue extends OptionsStruct implements Structure.ByValue {
        }

        // https://github.com/PRQL/prql/blob/5eaf6bcf897c062e1efaff87c5239bbd637ad526/bindings/prql-lib/src/lib.rs#L122-L138
        public boolean format = true; // NOSONAR
        public String target = "sql.any"; // NOSONAR
        public boolean signature_comment = true; // NOSONAR
    }

    @FieldOrder({ "start_line", "start_col", "end_line", "end_col" })
    static class SourceLocationStruct extends Structure { // NOSONAR
        public static class ByValue extends SourceLocationStruct implements Structure.ByValue {
        }

        public long start_line; // NOSONAR
        public long start_col; // NOSONAR
        public long end_line; // NOSONAR
        public long end_col; // NOSONAR
    }

    @FieldOrder({ "start", "end" })
    static class SpanStruct extends Structure { // NOSONAR
        public static class ByValue extends SpanStruct implements Structure.ByValue {
        }

        public long start; // NOSONAR
        public long end; // NOSONAR
    }

    @FieldOrder({ "kind", "code", "reason", "hint", "span", "display", "location" })
    static class MessageStruct extends Structure { // NOSONAR
        public static class ByReference extends MessageStruct implements Structure.ByReference {
        }

        public int kind; // NOSONAR
        public Pointer code; // NOSONAR
        public String reason; // NOSONAR
        public Pointer hint; // NOSONAR
        public SpanStruct.ByValue span; // NOSONAR
        public Pointer display; // NOSONAR
        public SourceLocationStruct.ByValue location; // NOSONAR
    }

    @FieldOrder({ "output", "messages", "messages_len" })
    static class CompileResultStruct extends Structure { // NOSONAR
        public static class ByValue extends CompileResultStruct implements Structure.ByValue {
        }

        public String output; // NOSONAR
        public MessageStruct.ByReference messages; // NOSONAR
        public long messages_len; // NOSONAR

        boolean hasError() {
            return (output == null || output.isEmpty()) && messages_len > 0L;
        }

        String getErrorMessage() {
            final int len = (int) messages_len;
            if (messages == null || len < 1) {
                return "";
            }

            MessageStruct.ByReference[] msgs = (MessageStruct.ByReference[]) messages.toArray(len);
            if (len == 1) {
                return msgs[0].reason;
            }

            StringBuilder builder = new StringBuilder();
            for (int i = 0; i < len; i++) {
                builder.append('\n').append(i + 1).append(") ").append(msgs[i].reason);
            }
            return builder.toString();
        }
    }

    /**
     * Compiles a PRQL string into a SQL string.
     * {@link #result_destroy(org.prql_lang.jna.PrqlLibrary.CompileResultStruct.ByValue)}
     * must be called to release memory allocated for
     * {@link CompileResultStruct.ByValue}.
     *
     * @param prql_query PRQL string
     * @param options    compile options
     * @return compile result
     */
    CompileResultStruct.ByValue compile(String prql_query, OptionsStruct.ByValue options); // NOSONAR

    /**
     * Builds PL AST from a PRQL string.
     * {@link #result_destroy(org.prql_lang.jna.PrqlLibrary.CompileResultStruct.ByValue)}
     * must be called to release memory allocated for
     * {@link CompileResultStruct.ByValue}.
     *
     * @param prql_query PRQL string
     * @return compile result
     */
    CompileResultStruct.ByValue prql_to_pl(String prql_query); // NOSONAR

    /**
     * Finds variable references, validates functions calls, determines frames and
     * converts PL to RQ.
     * {@link #result_destroy(org.prql_lang.jna.PrqlLibrary.CompileResultStruct.ByValue)}
     * must be called to release memory allocated for
     * {@link CompileResultStruct.ByValue}.
     *
     * @param pl_json PL AST in JSON string
     * @return compile result
     */
    CompileResultStruct.ByValue pl_to_rq(String pl_json); // NOSONAR

    /**
     * Convert RQ AST into a SQL string.
     * {@link #result_destroy(org.prql_lang.jna.PrqlLibrary.CompileResultStruct.ByValue)}
     * must be called to release memory allocated for
     * {@link CompileResultStruct.ByValue}.
     *
     * @param rq_json RQ AST in JSON string
     * @param options compile options
     * @return compile result
     */
    CompileResultStruct.ByValue rq_to_sql(String rq_json, OptionsStruct.ByValue options); // NOSONAR

    /**
     * Destroies a {@link CompileResultStruct.ByValue} once you are done with it.
     *
     * @param res compile result to destroy
     */
    void result_destroy(CompileResultStruct.ByValue res); // NOSONAR
}