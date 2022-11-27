package org.prql.prql4j;

import org.junit.Test;

public class PrqlCompilerTest {
    @Test
    public void compile() {
        String sql = "SELECT\n" +
                "  *\n" +
                "FROM\n" +
                "  table";
        assert sql.equalsIgnoreCase(PrqlCompiler.toSql("from table"));
    }
}
