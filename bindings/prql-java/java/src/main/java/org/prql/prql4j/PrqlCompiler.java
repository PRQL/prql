package org.prql.prql4j;

import java.io.IOException;

public class PrqlCompiler {

    /**
     * compile PRQL to SQL
     * @param query PRQL query
     * @param target target dialect, such as sql.mysql etc. Please refer <a href="https://github.com/PRQL/prql/blob/main/web/book/src/project/target.md">PRQL Target and Version</a>
     * @param format format SQL or not
     * @param signature comment signature or not
     * @return SQL
     * @throws Exception PRQL compile exception
     */
    public static native String toSql(String query, String target, boolean format, boolean signature) throws Exception;
    public static native String toJson(String query) throws Exception;
    public static native String format(String query) throws Exception;

    static {
        try {
            NativeLibraryLoader.getInstance().loadLibrary(null);
        } catch (IOException e) {
            throw new RuntimeException(e);
        }
    }
}
