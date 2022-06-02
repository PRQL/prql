---
title: "Roadmap"
url: roadmap
---

I'm excited and inspired by the level of enthusiasm behind the project, both
from individual contributors and the broader community of users who are
unsatisfied with SQL. We currently have an initial working version for the
intrepid early user.

I'm hoping we can build a beautiful language, an app that's approachable &
powerful, and a vibrant community. Many projects have reached the current stage
and fallen, so this requires compounding on what we've done so far.

### Language design

Already since becoming public, the language has improved dramatically, thanks to
the feedback of dozens of contributors. The current state of the basics is now
stable and while we'll hit corner-cases, I expect we'll only make small changes
to the existing features — even as we continue adding features.

Feel free to post questions or continue discussions on [Language Design
Issues](https://github.com/prql/prql/issues?q=is%3Aissue+is%3Aopen+label%3Alanguage-design).

### Documentation

Currently the language documentation is at <https://prql-lang.org/book>.

If you're up for contributing and don't have a preference for writing code or
not, this is the area that would most benefit from your contribution. Issues are
tagged with
[documentation](https://github.com/prql/prql/labels/documentation).

### Friendliness

Currently the language implementation is not sufficiently friendly, despite
significant recent improvements. We'd like to make error messages better, sand
off sharp corners, etc.

Both bug reports of unfriendliness, and code contributions to improve them are
welcome; there's a
[friendliness](https://github.com/prql/prql/issues?q=is%3Aissue+label%3Afriendlienss+is%3Aopen)
label.

### Fast feedback

As well as a command-line tool that transpiles queries, we'd like to make
developing in PRQL a wonderful experience, where it feels like it's on your
side:

- Syntax highlighting in more editors; currently we have a basic [VSCode
  extension](https://github.com/prql/prql-code).
- Initial type-inference, where it's possible without connecting to the DB, e.g.
  [#55](https://github.com/prql/prql/pull/55).
- Improvements or integrations to the [live in-browser
  compiler](https://prql-lang.org/playground/), including querying actual
  tables.
- (I'm sure there's more, ideas welcome)

### Integrations

PRQL is focused at the language layer, which means we can easily integrate with
existing tools & apps. This will be the primary way that people can start using
PRQL day-to-day. Probably the most impactful initial integrations will be tools that
engineers use to build data pipelines, like
[`dbt-prql`](https://github.com/prql/prql/issues/375).

### Database cohesion

One benefit of PRQL over SQL is that auto-complete, type-inference, and
error checking can be much more powerful.

We'd like to build this out. It's more difficult to build, since it requires a
connection to the database in order to understand the schema of the table.

### Not in focus

We should focus on solving a distinct problem really well. PRQL's goal is to
make reading and writing analytical queries easier, and so for the moment that
means putting some things out of scope:

- Building infrastructure outside of queries, like lineage. dbt is excellent at
  that! ([#13](https://github.com/prql/prql/issues/13)).
- Writing DDL / index / schema manipulation / inserting data
  ([#16](https://github.com/prql/prql/issues/16)).
