package org.prql.prql4j;

import org.junit.Test;

public class PrqlCompilerTest {
    @Test
    public void compile() throws Exception {
        String found = PrqlCompiler.toSql("from my_table", "sql.mysql", true, true);

        // remove signature
        found = found.substring(0, found.indexOf("\n\n--"));

        String expected = "SELECT\n" +
                "  *\n" +
                "FROM\n" +
                "  my_table";
        assert expected.equalsIgnoreCase(found);
    }

    @Test(expected = Exception.class)
    public void compileWithError() throws Exception {
       PrqlCompiler.toSql("from table | filter id >> 1", "sql.mysql", true, true);
    }
}
