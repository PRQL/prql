# development description for prql-java module

---

## Implementation

We implement Rust bindings to Java with
[JNI](https://docs.oracle.com/javase/8/docs/technotes/guides/jni/).

First, define a native method --
`public static native String toSql(String query, String target, boolean format, boolean signature)`
for PrqlCompiler, `toJson` is same.

And then implement it in Rust with this
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

To publish the Java library to Maven public repo, project maintainer need first
register a project in the Maven Nexus repository, by the doc:
<https://central.sonatype.org/publish/publish-guide/>.

And then, we can release our artifact in the `release` workflow. The action we
used is
[action-maven-publish](https://github.com/marketplace/actions/action-maven-publish).
Project maintainer has to configure some personal information, those used in the
first step, by the action's doc, such as `nexus_username`, `nexus_password`,
`gpg_private_key`, `gpg_passphrase`.
