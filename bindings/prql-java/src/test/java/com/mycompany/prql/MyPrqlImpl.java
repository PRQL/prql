package com.mycompany.prql;

import java.util.Properties;

import org.prql_lang.CompileOptions;
import org.prql_lang.Prql;
import org.prql_lang.PrqlException;

public class MyPrqlImpl extends Prql.Factory implements Prql {

    @Override
    public String compile(String prql, CompileOptions options) throws PrqlException {
        return prql;
    }

    @Override
    public String prql2pl(String prql) throws PrqlException {
        return prql;
    }

    @Override
    public String pl2rq(String plJson) throws PrqlException {
        return plJson;
    }

    @Override
    public String rq2sql(String rqJson, CompileOptions options) throws PrqlException {
        return rqJson;
    }

    @Override
    protected boolean accept(String api) {
        return "custom".equalsIgnoreCase(api);
    }

    @Override
    protected Prql get(Properties config) {
        return this;
    }
}
