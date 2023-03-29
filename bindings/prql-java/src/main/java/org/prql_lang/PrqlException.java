package org.prql_lang;

public class PrqlException extends Exception {
    private static final long serialVersionUID = 2916779227180717114L;

    public PrqlException(String message) {
        super(message);
    }

    public PrqlException(Throwable cause) {
        super(cause);
    }

    public PrqlException(String message, Throwable cause) {
        super(message, cause);
    }
}