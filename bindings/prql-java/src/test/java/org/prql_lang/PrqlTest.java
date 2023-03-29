package org.prql_lang;

import java.util.Properties;
import java.util.UUID;

import org.testng.Assert;
import org.testng.annotations.Test;

public class PrqlTest {
    @Test
    public void testFactory() throws PrqlException {
        Assert.assertEquals(Prql.Factory.factories.size(), 2);
        Assert.assertEquals(Prql.Factory.getInstance(), Prql.Factory.factories.get(1));
        Assert.assertEquals(Prql.Factory.getInstance(null), Prql.Factory.factories.get(1));

        Properties properties = new Properties();
        Assert.assertEquals(Prql.Factory.getInstance(null), Prql.Factory.getInstance(properties));
        properties.setProperty("prql.api", "cli");
        Assert.assertThrows(IllegalStateException.class, () -> Prql.Factory.getInstance(properties));
        properties.setProperty("prql.api", "custom");
        Assert.assertEquals(Prql.Factory.getInstance(properties), Prql.Factory.factories.get(0));

        String query = UUID.randomUUID().toString();
        Assert.assertEquals(Prql.getInstance(properties).compile(query, null), query);
        Assert.assertEquals(Prql.getInstance(properties).compile(query, CompileOptions.DEFAULT), query);
        Assert.assertEquals(Prql.getInstance(properties).prql2pl(query), query);
        Assert.assertEquals(Prql.getInstance(properties).pl2rq(query), query);
        Assert.assertEquals(Prql.getInstance(properties).rq2sql(query, null), query);
        Assert.assertEquals(Prql.getInstance(properties).rq2sql(query, CompileOptions.COMPACT), query);
    }

    @Test
    public void testGetInstance() {
        Assert.assertEquals(Prql.getInstance(), Prql.getInstance(null));
        Assert.assertEquals(Prql.getInstance(), Prql.Factory.getInstance().get(null));

        Properties properties = new Properties();
        Assert.assertEquals(Prql.getInstance(), Prql.getInstance(properties));
        Assert.assertEquals(Prql.getInstance(), Prql.Factory.getInstance().get(properties));
        properties.setProperty("prql.api", "cli");
        Assert.assertThrows(IllegalStateException.class, () -> Prql.getInstance(properties));
    }

    @Test
    public void testCompile() throws PrqlException {
        Assert.assertThrows(PrqlException.class, () -> Prql.getInstance().compile(null, null));
        Assert.assertThrows(PrqlException.class, () -> Prql.getInstance().compile("", null));
        Assert.assertThrows(PrqlException.class, () -> Prql.getInstance().compile(" \r\n\t", null));
        Assert.assertThrows(PrqlException.class, () -> Prql.getInstance().compile("invalid prql", null));
        Assert.assertThrows(PrqlException.class,
                () -> Prql.getInstance().compile("prql target:mssql\nfrom test | take 10", null));

        Assert.assertThrows(PrqlException.class, () -> Prql.getInstance().compile(null, CompileOptions.DEFAULT));
        Assert.assertThrows(PrqlException.class, () -> Prql.getInstance().compile("", CompileOptions.DEFAULT));
        Assert.assertThrows(PrqlException.class, () -> Prql.getInstance().compile(" \r\n\t", CompileOptions.DEFAULT));
        Assert.assertThrows(PrqlException.class,
                () -> Prql.getInstance().compile("invalid prql", CompileOptions.DEFAULT));
        Assert.assertThrows(PrqlException.class,
                () -> Prql.getInstance().compile("prql target:mssql\nfrom test | take 10", CompileOptions.DEFAULT));

        CompileOptions options = CompileOptions.COMPACT;
        Assert.assertEquals(Prql.getInstance().compile("from test | take 10", options), "SELECT * FROM test LIMIT 10");
        Assert.assertEquals(Prql.getInstance().compile("prql target:sql.mysql\nfrom test | take 10", options),
                "SELECT * FROM test LIMIT 10");
        Assert.assertEquals(Prql.getInstance().compile("prql target:sql.mssql\nfrom test | take 10", options),
                "SELECT TOP (10) * FROM test");

        options = new CompileOptions(false, SqlDialect.MSSQL, false);
        Assert.assertEquals(Prql.getInstance().compile("from test | take 10", options), "SELECT TOP (10) * FROM test");
        Assert.assertEquals(Prql.getInstance().compile("prql target:sql.mysql\nfrom test | take 10", options),
                "SELECT TOP (10) * FROM test");

        options = new CompileOptions(true, SqlDialect.MARIADB, false);
        Assert.assertEquals(Prql.getInstance().compile("from test | take 10", options),
                "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n");
        Assert.assertEquals(Prql.getInstance().compile("prql target:sql.mssql\nfrom test | take 10", options),
                "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n");

        options = new CompileOptions(true, SqlDialect.MARIADB, true);
        Assert.assertTrue(Prql.getInstance().compile("from test | take 10", options).startsWith(
                "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n\n-- "));
        Assert.assertTrue(Prql.getInstance().compile("prql target:sql.mssql\nfrom test | take 10", options).startsWith(
                "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n\n-- "));
    }

    @Test
    public void testConversion() throws PrqlException {
        for (String invalidInput : new String[] { null, "", " ", "invalid input" }) {
            if (invalidInput == null) {
                Assert.assertThrows(PrqlException.class, () -> Prql.getInstance().prql2pl(invalidInput));
            } else {
                Assert.assertTrue(Prql.getInstance().prql2pl(invalidInput).length() > 0);
            }
            Assert.assertThrows(PrqlException.class, () -> Prql.getInstance().pl2rq(invalidInput));
            Assert.assertThrows(PrqlException.class, () -> Prql.getInstance().rq2sql(invalidInput, null));
            Assert.assertThrows(PrqlException.class,
                    () -> Prql.getInstance().rq2sql(invalidInput, CompileOptions.DEFAULT));
        }

        Prql api = Prql.getInstance();
        Assert.assertEquals(
                api.rq2sql(api.pl2rq(api.prql2pl("from test | take 10")), CompileOptions.COMPACT),
                "SELECT * FROM test LIMIT 10");
        Assert.assertEquals(
                api.rq2sql(api.pl2rq(api.prql2pl("from test | take 10")),
                        new CompileOptions(false, SqlDialect.MSSQL, false)),
                "SELECT TOP (10) * FROM test");
        Assert.assertEquals(
                api.rq2sql(api.pl2rq(api.prql2pl("prql target:sql.mssql\nfrom test | take 10")),
                        CompileOptions.COMPACT),
                "SELECT TOP (10) * FROM test");
    }
}