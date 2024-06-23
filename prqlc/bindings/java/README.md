# prql-java

`prql-java` offers Java bindings to the `prqlc` Rust library. It exposes a Java
native method `public static native String toSql(String query)`.

It's still at an early stage, and currently requires compiling locally, and
isn't published to Maven. Contributions are welcome.

## Installation

```xml
<dependency>
    <groupId>org.prqllang</groupId>
    <artifactId>prql-java</artifactId>
    <version>${PRQL_VERSION}</version>
</dependency>
```

## Usage

```java
import org.prqllang.prql4j.PrqlCompiler;

class Main {
    public static void main(String[] args) {
        String sql = PrqlCompiler.toSql("from table");
        System.out.println(sql);
    }
}
```
