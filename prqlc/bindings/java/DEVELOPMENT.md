# development description for prql-java module

---

## Implementation

We implement Rust bindings to Java with
[JNI](https://docs.oracle.com/javase/8/docs/technotes/guides/jni/).

First, define the native methods on `PrqlCompiler` --
`toSql(String query, String target, boolean format, boolean signature)`,
`toJson(String query)` and `format(String query)`.

And then implement them in Rust with this
[crate](https://docs.rs/jni/latest/jni/).

## Build

For ease of use to users, we need pre-build dynamic libs for different
platforms. This process is combined into the build of Java module.

We use [Maven](https://maven.apache.org/) to build the Java library. To add the
Rust cross compilation into the Maven build process, we add the following XML
segment to the `pom.xml`:

```xml
<plugin>
    <artifactId>exec-maven-plugin</artifactId>
    <groupId>org.codehaus.mojo</groupId>
    <version>1.6.0</version>
    <executions>
        <execution>
            <id>Build for release</id>
            <phase>generate-resources</phase>
            <goals>
                <goal>exec</goal>
            </goals>
            <configuration>
                <executable>../cross.sh</executable>
                <arguments>
                    <argument>${project.basedir}/../</argument>
                </arguments>
            </configuration>
        </execution>
    </executions>
</plugin>
```

When we build, it will execute the `cross.sh` script to get all the Rust
cdylibs. This process is time consuming.

As to cross compilation toolchains, we use
[cross](https://github.com/cross-rs/cross).

## Publish (for maintainer)

**Publishing is not currently wired up.** The `publish-prql-java` job in
`.github/workflows/release.yaml` is commented out, and no `org.prqllang`
artifact exists on Maven Central.
[#850](https://github.com/PRQL/prql/issues/850) tracks the remaining work: the
job failed on a missing `distribution` argument, the Maven auth tokens are
unconfigured, and `java/pom.xml` carries a hand-maintained `<version>` that no
release step bumps.

The commented-out job is the starting point rather than a working recipe. To
finish it, a maintainer would first register the project in the Maven Nexus
repository, by the doc: <https://central.sonatype.org/publish/publish-guide/>,
then configure the secrets the action needs -- `nexus_username`,
`nexus_password`, `gpg_private_key`, `gpg_passphrase` -- and correct the job's
stale `directory:`, which still points at the pre-`prqlc/bindings` layout.
