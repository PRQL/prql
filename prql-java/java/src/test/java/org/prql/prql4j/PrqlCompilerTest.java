package org.prql.prql4j;

import org.junit.Test;

public class PrqlCompilerTest {
    @Test
    public void compile() {
        String found = PrqlCompiler.toSql("from table");

        // remove signature
        found = found.substring(0, found.indexOf("\n\n--"));

        String expected = "SELECT\n" +
                "  *\n" +
                "FROM\n" +
                "  table";
        assert expected.equalsIgnoreCase(found);
    }
}
