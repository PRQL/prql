package org.prql_lang.jna;

import java.util.function.Supplier;

import org.prql_lang.jna.PrqlLibrary.CompileResultStruct;
import org.prql_lang.jna.PrqlLibrary.MessageStruct;
import org.prql_lang.jna.PrqlLibrary.OptionsStruct;
import org.testng.Assert;
import org.testng.annotations.BeforeClass;
import org.testng.annotations.Test;

import com.sun.jna.Native;

public class PrqlLibraryTest {
    private PrqlLibrary lib = null;

    @BeforeClass
    private void setup() {
        lib = Native.load("prql_lib", PrqlLibrary.class);
    }

    private void checkCompileOutput(Supplier<CompileResultStruct.ByValue> func, String output) {
        CompileResultStruct.ByValue result = null;
        try {
            result = func.get();
            Assert.assertNotNull(result.output, "Output should never be null");
            if (output != null && output.endsWith("-- ")) {
                Assert.assertTrue(result.output.startsWith(output));
            } else {
                Assert.assertEquals(result.output, output);
            }
            if (result.messages == null) { // success
                Assert.assertEquals(result.messages_len, 0L);
            } else {
                Assert.assertEquals(result.output, "");
                Assert.assertTrue(result.messages_len > 0L, "Result should contain error message(s)");
                MessageStruct.ByReference[] messages = (MessageStruct.ByReference[]) result.messages
                        .toArray((int) result.messages_len);
                Assert.assertEquals(messages.length + 0L, result.messages_len);
                for (MessageStruct.ByReference ref : ((MessageStruct.ByReference[]) result.messages
                        .toArray((int) result.messages_len))) {
                    Assert.assertTrue(ref.kind >= 0 && ref.kind <= 2);
                    Assert.assertTrue(ref.reason.length() > 0);
                    Assert.assertNotNull(ref.span);
                    Assert.assertNotNull(ref.location);
                }
            }
        } finally {
            if (result != null) {
                lib.result_destroy(result);
            }
        }
    }

    @Test
    public void testCompile() {
        // JVM panic
        // Assert.expectThrows(Throwable.class, () -> lib.compile(null, null));

        checkCompileOutput(() -> lib.compile("", null), "");
        checkCompileOutput(() -> lib.compile("invalid prql", null), "");

        OptionsStruct.ByValue options = new OptionsStruct.ByValue();
        Assert.assertTrue(options.format);
        Assert.assertEquals(options.target, "sql.any");
        Assert.assertTrue(options.signature_comment);

        options.format = false;
        options.signature_comment = false;

        checkCompileOutput(() -> lib.compile("", options), "");
        checkCompileOutput(() -> lib.compile("invalid prql", options), "");
        checkCompileOutput(() -> lib.compile("from test | take 10", options), "SELECT * FROM test LIMIT 10");
        checkCompileOutput(() -> lib.compile("prql target:sql.mssql\nfrom test | take 10", options),
                "SELECT TOP (10) * FROM test");
        checkCompileOutput(() -> lib.compile("prql target:mssql\nfrom test | take 10", options), "");
        checkCompileOutput(() -> lib.compile("prql target:sql.mysql\nfrom test | take 10", options),
                "SELECT * FROM test LIMIT 10");

        options.target = "sql.mssql";
        checkCompileOutput(() -> lib.compile("from test | take 10", options), "SELECT TOP (10) * FROM test");
        checkCompileOutput(() -> lib.compile("prql target:mssql\nfrom test | take 10", options),
                "SELECT TOP (10) * FROM test");
        checkCompileOutput(() -> lib.compile("prql target:sql.mysql\nfrom test | take 10", options),
                "SELECT TOP (10) * FROM test");

        options.target = "mssql";
        checkCompileOutput(() -> lib.compile("from test | take 10", options), "");
        checkCompileOutput(() -> lib.compile("prql target:sql.mysql\nfrom test | take 10", options), "");

        options.target = "sql.mysql";
        checkCompileOutput(() -> lib.compile("from test | take 10", options), "SELECT * FROM test LIMIT 10");
        checkCompileOutput(() -> lib.compile("prql target:mssql\nfrom test | take 10", options),
                "SELECT * FROM test LIMIT 10");
        checkCompileOutput(() -> lib.compile("prql target:sql.mssql\nfrom test | take 10", options),
                "SELECT * FROM test LIMIT 10");

        options.signature_comment = true;
        checkCompileOutput(() -> lib.compile("from test | take 10", options), "SELECT * FROM test LIMIT 10 -- ");
        checkCompileOutput(() -> lib.compile("prql target:mssql\nfrom test | take 10", options),
                "SELECT * FROM test LIMIT 10 -- ");
        checkCompileOutput(() -> lib.compile("prql target:sql.mssql\nfrom test | take 10", options),
                "SELECT * FROM test LIMIT 10 -- ");

        options.signature_comment = false;
        options.format = true;
        checkCompileOutput(() -> lib.compile("from test | take 10", options),
                "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n");
        checkCompileOutput(() -> lib.compile("prql target:mssql\nfrom test | take 10", options),
                "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n");
        checkCompileOutput(() -> lib.compile("prql target:sql.mssql\nfrom test | take 10", options),
                "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n");

        options.signature_comment = true;
        checkCompileOutput(() -> lib.compile("from test | take 10", options),
                "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n\n-- ");
        checkCompileOutput(() -> lib.compile("prql target:mssql\nfrom test | take 10", options),
                "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n\n-- ");
        checkCompileOutput(() -> lib.compile("prql target:sql.mssql\nfrom test | take 10", options),
                "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n\n-- ");

        options.target = "sql.any";
        checkCompileOutput(() -> lib.compile("from test | take 10", options),
                "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n\n-- ");
        checkCompileOutput(() -> lib.compile("prql target:mssql\nfrom test | take 10", options), "");
        checkCompileOutput(() -> lib.compile("prql target:sql.mssql\nfrom test | take 10", options),
                "SELECT\n  TOP (10) *\nFROM\n  test\n\n-- ");
    }

    @Test
    public void testPrql2Pl() {
        // JVM panic
        // Assert.expectThrows(Throwable.class, () -> lib.prql_to_pl(null));

        checkCompileOutput(() -> lib.prql_to_pl(""), "[]");
        checkCompileOutput(() -> lib.prql_to_pl(" "), "[]");
        checkCompileOutput(() -> lib.prql_to_pl("invalid prql"), "");
        checkCompileOutput(() -> lib.prql_to_pl("from test | take 10"),
                "[{\"Main\":{\"Pipeline\":{\"exprs\":[{\"FuncCall\":{\"name\":{\"Ident\":[\"from\"]},\"args\":[{\"Ident\":[\"test\"]}]}},{\"FuncCall\":{\"name\":{\"Ident\":[\"take\"]},\"args\":[{\"Literal\":{\"Integer\":10}}]}}]}}}]");
    }

    @Test
    public void testPl2Rq() {
        // JVM panic
        // Assert.expectThrows(Throwable.class, () -> lib.prql_to_pl(null));

        checkCompileOutput(() -> lib.pl_to_rq(""), "");
        checkCompileOutput(() -> lib.pl_to_rq(" "), "");
        checkCompileOutput(() -> lib.pl_to_rq("invalid pl(not json)"), "");
        checkCompileOutput(() -> lib.pl_to_rq(
                "[{\"Main\":{\"Pipeline\":{\"exprs\":[{\"FuncCall\":{\"name\":{\"Ident\":[\"from\"]},\"args\":[{\"Ident\":[\"test\"]}]}},{\"FuncCall\":{\"name\":{\"Ident\":[\"take\"]},\"args\":[{\"Literal\":{\"Integer\":10}}]}}]}}}]"),
                "{\"def\":{\"version\":null,\"other\":{}},\"tables\":[{\"id\":0,\"name\":null,\"relation\":{\"kind\":{\"ExternRef\":{\"LocalTable\":\"test\"}},\"columns\":[\"Wildcard\"]}}],\"relation\":{\"kind\":{\"Pipeline\":[{\"From\":{\"source\":0,\"columns\":[[\"Wildcard\",0]],\"name\":\"test\"}},{\"Take\":{\"range\":{\"start\":null,\"end\":{\"kind\":{\"Literal\":{\"Integer\":10}},\"span\":null}},\"partition\":[],\"sort\":[]}},{\"Select\":[0]}]},\"columns\":[\"Wildcard\"]}}");
    }

    @Test
    public void testRq2Sql() {
        // JVM panic
        // Assert.expectThrows(Throwable.class, () -> lib.rq_to_sql(null, null));

        checkCompileOutput(() -> lib.rq_to_sql("", null), "");
        checkCompileOutput(() -> lib.rq_to_sql(" ", null), "");
        checkCompileOutput(() -> lib.rq_to_sql("invalid rq(not json)", null), "");
        final String rq = "{\"def\":{\"version\":null,\"other\":{}},\"tables\":[{\"id\":0,\"name\":null,\"relation\":{\"kind\":{\"ExternRef\":{\"LocalTable\":\"test\"}},\"columns\":[\"Wildcard\"]}}],\"relation\":{\"kind\":{\"Pipeline\":[{\"From\":{\"source\":0,\"columns\":[[\"Wildcard\",0]],\"name\":\"test\"}},{\"Take\":{\"range\":{\"start\":null,\"end\":{\"kind\":{\"Literal\":{\"Integer\":10}},\"span\":null}},\"partition\":[],\"sort\":[]}},{\"Select\":[0]}]},\"columns\":[\"Wildcard\"]}}";
        checkCompileOutput(() -> lib.rq_to_sql(rq, null), "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n\n-- ");

        OptionsStruct.ByValue options = new OptionsStruct.ByValue();
        Assert.assertTrue(options.format);
        Assert.assertEquals(options.target, "sql.any");
        Assert.assertTrue(options.signature_comment);

        options.format = false;
        options.signature_comment = false;

        checkCompileOutput(() -> lib.rq_to_sql("", options), "");
        checkCompileOutput(() -> lib.rq_to_sql(" ", options), "");
        checkCompileOutput(() -> lib.rq_to_sql("invalid prql(not json)", options), "");
        checkCompileOutput(() -> lib.rq_to_sql(rq, options), "SELECT * FROM test LIMIT 10");

        options.target = "sql.mssql";
        checkCompileOutput(() -> lib.rq_to_sql(rq, options), "SELECT TOP (10) * FROM test");

        options.target = "mssql";
        checkCompileOutput(() -> lib.rq_to_sql(rq, options), "");

        options.target = "sql.mysql";
        checkCompileOutput(() -> lib.rq_to_sql(rq, options), "SELECT * FROM test LIMIT 10");

        options.signature_comment = true;
        checkCompileOutput(() -> lib.rq_to_sql(rq, options), "SELECT * FROM test LIMIT 10 -- ");

        options.signature_comment = false;
        options.format = true;
        checkCompileOutput(() -> lib.rq_to_sql(rq, options), "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n");

        options.signature_comment = true;
        checkCompileOutput(() -> lib.rq_to_sql(rq, options), "SELECT\n  *\nFROM\n  test\nLIMIT\n  10\n\n-- ");
    }
}
