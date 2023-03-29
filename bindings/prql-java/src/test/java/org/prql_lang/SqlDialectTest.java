package org.prql_lang;

import org.testng.Assert;
import org.testng.annotations.Test;

public class SqlDialectTest {
    @Test
    public void testOf() {
        Assert.assertEquals(SqlDialect.of(null), SqlDialect.ANY);
        Assert.assertEquals(SqlDialect.of(""), SqlDialect.ANY);
        Assert.assertEquals(SqlDialect.of("unknown.dialect"), SqlDialect.ANY);

        for (SqlDialect d : SqlDialect.values()) {
            Assert.assertNotNull(SqlDialect.of(d.getKey()), "Should not have problem with " + d.getKey());
            Assert.assertEquals(SqlDialect.of(d.getKey()).getKey(), d.getKey());
            // without prefix
            Assert.assertEquals(SqlDialect.of(d.getKey().substring(SqlDialect.PREFIX.length())).getKey(), d.getKey());
        }
    }
}
