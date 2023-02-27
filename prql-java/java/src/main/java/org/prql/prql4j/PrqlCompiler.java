package org.prql.prql4j;

import java.io.IOException;

public class PrqlCompiler {
    public static native String toSql(String query) throws Exception;
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
