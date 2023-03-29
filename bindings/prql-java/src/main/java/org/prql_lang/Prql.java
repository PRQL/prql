package org.prql_lang;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedList;
import java.util.List;
import java.util.Locale;
import java.util.Properties;
import java.util.ServiceLoader;

/**
 * Java bindings of PRQL Rust library.
 */
public interface Prql {
    /**
     * Factory class.
     */
    abstract static class Factory {
        static final List<Factory> factories;

        static {
            String prefix = Factory.class.getPackage().getName() + ".";
            LinkedList<Factory> list = new LinkedList<>();
            for (Factory f : ServiceLoader.load(Factory.class)) {
                if (f.getClass().getName().startsWith(prefix)) {
                    list.addLast(f);
                } else {
                    // customized implementation takes priority
                    list.addFirst(f);
                }
            }
            factories = Collections.unmodifiableList(new ArrayList<>(list));
        }

        /**
         * Gets the default factory. Same as passing {@code null} or properties with
         * {@code prql.api = ""} to {@link #getInstance(Properties)}.
         *
         * @return non-null factory instance
         * @throws IllegalStateException when failed to get the default factory
         */
        public static final Factory getInstance() {
            return getInstance(null);
        }

        /**
         * Gets factory instance according to the given configuration.
         *
         * @param config optional configuration with hints, {@link #accept(String)} will
         *               be called against property {@code prql.api} to ensure only
         *               suitable factory instance will be returned
         * @return non-null factory instance
         * @throws IllegalStateException when failed to get factory instance
         */
        public static final Factory getInstance(Properties config) {
            final String api = config == null ? "" : config.getProperty("prql.api", "");
            for (Factory f : factories) {
                if (f.accept(api)) {
                    return f;
                }
            }

            throw new IllegalStateException(String.format(Locale.ROOT, "No implementation of %s is available",
                    Factory.class.getName()));
        }

        /**
         * Checks whether the given {@code api} is supported by current implementation
         * or not.
         *
         * @param api non-null case insensitive API name, for examples:
         *            {@code jna}, {@code jni}, {@code cli}, {@code ffm},
         *            {@code custom}, or even an empty string, which is the default
         * @return true if the API is supported; false otherwise
         */
        protected abstract boolean accept(String api);

        /**
         * Gets an instance according to the given configuration. It's up to the factory
         * implementation on whether a new instance should be created or not.
         *
         * @param config optional configuration
         * @return non-null instance
         */
        protected abstract Prql get(Properties config);
    }

    /**
     * Gets an instance to handle PRQL compilation.
     *
     * @return non-null instance to handle PRQL compilation
     */
    static Prql getInstance() {
        return Factory.getInstance().get(null);
    }

    /**
     * Gets an instance to handle PRQL compilation.
     *
     * @param config optional configuration
     * @return non-null instance to handle PRQL compilation
     */
    static Prql getInstance(Properties config) {
        return Factory.getInstance(config).get(config);
    }

    /**
     * Compiles a PRQL string into a SQL string.
     *
     * @param prql    non-null PRQL string
     * @param options optional compile option
     * @return non-null SQL string
     * @throws PrqlException when failed to compile the given query
     */
    String compile(String prql, CompileOptions options) throws PrqlException;

    /**
     * Converts PRQL string to PL AST.
     *
     * @param prql non-null PRQL string
     * @return non-null PL AST in JSON string
     * @throws PrqlException when failed to compile the given query
     */
    String prql2pl(String prql) throws PrqlException;

    /**
     * Converts PL to RQ.
     *
     * @param plJson non-null PL in JSON string
     * @return non-null RQ AST in JSON string
     * @throws PrqlException when failed to compile the given query
     */
    String pl2rq(String plJson) throws PrqlException;

    /**
     * Converts RQ to SQL.
     *
     * @param rqJson  non-null RQ in JSON string
     * @param options compile options, could be null
     * @return non-null SQL
     * @throws PrqlException when failed to compile the given query
     */
    String rq2sql(String rqJson, CompileOptions options) throws PrqlException;
}
