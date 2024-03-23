{{#include ../../../../../grammars/README.md}}

---

Since the [Elm](https://elm-lang.org/) language coincidentally provides syntax
highlighting suitable for PRQL, it may look better to mark PRQL code as Elm when
the above definition files are not available.

For example, the following Markdown code block will be nicely highlighted on
GitHub, Pandoc, and other Markdown renderers:

````markdown
```elm
from employees
filter start_date > @2021-01-01
```
````

We hope that in the future these renderers will recognize PRQL code blocks and
have syntax highlighting applied, and we are tracking these with several issues.

- GitHub (Linguist): <https://github.com/PRQL/prql/issues/1636>
- Pandoc (Kate): <https://github.com/PRQL/prql/issues/2213>
