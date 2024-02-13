# Modules

> This is a technical document. For a "how to use" or a TLDR; skip to the
> [Example](#example) section.

Design goals:

1. Allow importing declarations from other files.

2. Have namespaces for things like `std`.

3. Have a hierarchical structure so we can represent files in directories.

4. Have an unambiguous module structure within a project.

## Definition

A module is a namespace that contains declarations. A module is itself a
declaration, which means that it can contain nested child modules.

This means that modules form a
[tree graph](<https://en.wikipedia.org/wiki/Tree_(graph_theory)>), which we call
"the module structure".

For the sake of this document, we will express the module structure with
`module` keyword and a code block encased in curly braces:

```
module my_playlists {
    let bangers = ... # a declaration

    module soundtracks {
        let movie_albums = ... # another declaration
    }
}
```

> The syntax `module name { ...decls... }` is not part of PRQL language, with
> the objection that it is unnecessary as it only adds more ways of defining
> modules. If a significant upside of this syntax is found, it may be added in
> the future.

## Name resolution

Any declarations within a module can be referenced from the outside of the
module:

```prql no-eval
# using module structure declared above
module my_playlists

let great_tracks = my_playlists.bangers

let movie_scores = my_playlists.soundtracks.movie_albums
```

Identifiers are resolved relative to current module.

```prql no-eval
module my_playlists {
    module soundtracks {
        let movie_albums = (from albums | filter id == 3)
    }

    from.soundtracks.movie_albums
}
from.my_playlists.soundtracks.movie_albums
```

If an identifier cannot be resolved relative to the current module, it tries to
resolve relative to the parent module. This is repeated, stepping up the module
hierarchy until a match is found or root of the module structure is reached.

```prql no-eval
module my_playlists {
    let decl_1 = ...

    module soundtracks {
        let decl_2 = ...
    }

    module upbeat_rock {
        let decl_3 = ...

        decl_1 | join soundtracks.decl2 | join decl_3
    }
}
```

## Main var declaration

The final variable declaration in a module can omit the leading `let main =` and
acquire an implicit name main.

```
module my_playlists {
    let bangers = (from.tracks | take 10)

    from.playlists | take 10
}

let album_titles = my_playlists.main
```

When a module is referenced as a value, the `main` variable is used instead.
This is especially useful when referring to a module which is to be compiled to
RQ (and later SQL).

```
# last line from previous example could thus be shortened to:
let album_titles = my_playlists
```

## File importing

> This section is under discussion. Current implementation plans do not include
> `module` declarations, but loading of all files under the compilation path.

To include PRQL source code from other files, we can use the following syntax:

```
module my_playlists
```

This loads either `./my_playlists.prql` (a leaf module) or
`./my_playlists/_my_playlists.prql` (a directory module) and uses its contents
as module `my_playlists`. If none or both of the files are present, a
compilation error is raised.

Only directory modules can contain module declarations. If a leaf module
contains a module declaration, a compilation error is raised, suggesting the
leaf module to be converted into a directory module. This is a step toward any
module structure having a single "normalized" representation in the file system.
Such normalization is desired because it restrains the possible file system
layouts to a comprehensible and predictable layout, while not sacrificing any
functionality.

Described importing rules don't achieve this "single normalized representation"
in full, since any leaf modules could be replaced by a directory module with
zero submodules, without any semantic changes. Restricting directory modules to
have at least one sub-module would not improve approachability enough to justify
adding this restriction.

For example, the following module structure is annotated with files names in
which the modules would reside:

```prql no-eval

module my_project {
    # _my_project.prql

    module sales {
        # sales.prql
    }

    module projections {
        # projections/_projections.prql

        module year_2023 {
            # projections/year_2023.prql
        }

        module year_2024 {
            # projections/year_2024.prql
        }
    }
}
```

If module `my_project.sales` wants to add a submodule `util`, it has to be
converted to a directory modules. This means that it has to be moved to
`sales/_sales.prql`. The new module would reside in `sales/util.prql`.

The annotated layout is not the only possible layout for this module structure,
since any of the modules `sales`, `year_2023` or `year_2024` could be converted
into a directory module with zero sub-modules.

Point 4 of design goals means that each declaration within a project has a
single fully-qualified name within this project. This is ensured by strict rules
regarding importing files and the fact that the module structure is a tree.

## Declaration order

The order of declarations in a module holds no semantic value, except the "last
`main` variable".

References between modules can be cyclic.

```
module mod_a {
    let decl_a_1 = ...
    let decl_a_2 = (from mod_b.decl_b | take 10)
}
module mod_b {
    let decl_b = (from mod_a.decl_a | take 10)
}
```

References between variable declarations cannot be cyclic.

```
let decl_a = (from decl_b)
let decl_b = (from decl_a) # error: cyclic reference
```

```
module mod_a {
    let decl_a = (from mod_b.decl_b)
}
module mod_b {
    let decl_b = (from mod_a.decl_a) # error: cyclic reference
}
```

## Compiler interface

`prqlc` provides two interfaces for compiling files.

**Multi-file interface** requires three arguments:

- path to the file containing the module which is the root of the module
  structure,
- identifier of the pipeline that should be compiled to RQ (this can also be an
  identifier of a module that has a `main` pipeline) and,
- a "file loader", which can load files on-demand.

The path to the root module can be automatically detected by searching for
`.prql` files starting with `_` in the current working directory.

Example prqlc usage:

```
$ prqlc compile _project.prql sales.projections.orders_2024
$ prqlc compile sales.projections.orders_2024
```

**Single-file interface** requires a single argument; the PRQL source. Any
attempts to load modules in this mode result in compilation errors. This
interface is needed, for example, when integrating the compiler with a database
connector (i.e. JDBC) where no other files can be loaded.

## Built-in module structure

> Work In Progress

```
# root module of every project
module project {
	module std {
		let sum = a -> ...
		let mean = a -> ...
	}

	module default_db {
		# all inferred tables and defined CTEs
	}

	let main = (
		from.tracks
		select {t = this}
		select [track_id, title]
	)
}
```

## Example

This is an example project, where each of code block is a separate file.

```
# _project.prql

module employees
module sales
module util
```

```
# employees.prql

let employees = (...)

let salaries = (...)

let departments = (...)
```

```
# sales/_sales.prql

module orders
module projections

let revenue_by_source = (...)
```

```
# sales/orders.prql

let current_year = (...)

let archived = (...)

let by_employee = (from orders | join employees.employees ...)
```

```
# sales/projections.prql

let orders_2023 = (from orders.current_year | append orders.archived)

let orders_2024 = (...)
```

```
# util.prql

let pretty_print_num = col -> (...)
```

---

Sources:

- [Notes On Module System](https://matklad.github.io/2021/11/27/notes-on-module-system.html),
  by @matklad.
