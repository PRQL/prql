package org.prql_lang.jna;

import java.util.Properties;

import com.sun.jna.Native;

import org.prql_lang.CompileOptions;
import org.prql_lang.Prql;
import org.prql_lang.PrqlException;
import org.prql_lang.jna.PrqlLibrary.CompileResultStruct;
import org.prql_lang.jna.PrqlLibrary.OptionsStruct;

/**
 * JNA implementation.
 */
public class JnaImpl extends Prql.Factory implements Prql {
    private static final PrqlLibrary lib;

    static {
        lib = Native.load("prql_lib", PrqlLibrary.class);
    }

    @Override
    protected boolean accept(String api) {
        return api == null || api.isEmpty() || "jna".equalsIgnoreCase(api);
    }

    @Override
    protected Prql get(Properties config) {
        return this;
    }

    @Override
    public String compile(String prql, CompileOptions options) throws PrqlException {
        if (prql == null || prql.isEmpty()) {
            throw new PrqlException("Non-empty PRQL is required");
        }

        OptionsStruct.ByValue optionsStruct = new OptionsStruct.ByValue();
        if (options != null) {
            optionsStruct.format = options.requireFormat();
            optionsStruct.target = options.getTarget();
            optionsStruct.signature_comment = options.requireComment();
        }
        CompileResultStruct.ByValue result = null;
        try {
            result = lib.compile(prql, optionsStruct);
            if (result.hasError()) {
                throw new PrqlException("Failed to compile PRQL due to:" + result.getErrorMessage());
            }
            return result.output;
        } finally {
            if (result != null) {
                lib.result_destroy(result);
            }
        }
    }

    @Override
    public String prql2pl(String prql) throws PrqlException {
        if (prql == null) {
            throw new PrqlException("Non-empty PRQL is required");
        }

        CompileResultStruct.ByValue result = null;
        try {
            result = lib.prql_to_pl(prql);
            if (result.hasError()) {
                throw new PrqlException("Failed to convert PRQL to PL due to:" + result.getErrorMessage());
            }
            return result.output;
        } finally {
            if (result != null) {
                lib.result_destroy(result);
            }
        }
    }

    @Override
    public String pl2rq(String plJson) throws PrqlException {
        if (plJson == null || plJson.isEmpty()) {
            throw new PrqlException("Non-empty PL is required");
        }

        CompileResultStruct.ByValue result = null;
        try {
            result = lib.pl_to_rq(plJson);
            if (result.hasError()) {
                throw new PrqlException("Failed to convert PL to RQ due to:" + result.getErrorMessage());
            }
            return result.output;
        } finally {
            if (result != null) {
                lib.result_destroy(result);
            }
        }
    }

    @Override
    public String rq2sql(String rqJson, CompileOptions options) throws PrqlException {
        if (rqJson == null || rqJson.isEmpty()) {
            throw new PrqlException("Non-empty RQ is required");
        }

        OptionsStruct.ByValue optionsStruct = new OptionsStruct.ByValue();
        if (options != null) {
            optionsStruct.format = options.requireFormat();
            optionsStruct.target = options.getTarget();
            optionsStruct.signature_comment = options.requireComment();
        }
        CompileResultStruct.ByValue result = null;
        try {
            result = lib.rq_to_sql(rqJson, optionsStruct);
            if (result.hasError()) {
                throw new PrqlException("Failed to convert RQ to SQL due to:" + result.getErrorMessage());
            }
            return result.output;
        } finally {
            if (result != null) {
                lib.result_destroy(result);
            }
        }
    }

}
