# Modules

PRQL needs modules that would:

- allow importing declarations from other files,
- have namespaces for things like `std`,
- have hierarchical structure so we can represent files in directories,
- have unambiguous module structure within a project,
- be able to compile individual files that are a part of a project.

## Declaration

Module is a namespace that contains declarations.

Modules can be defined with module keyword and a code block encased in curly
braces:

```
module my_module {
    let bangers = (from tracks | take 10)
}
```

## File loading

If a module definition of `my_module` does not have a code block, it's contents
are loaded from `./my_module.prql`.

If the file does not exist, contents are loaded from file
`./my_module/mod.prql`.

When compilation is invoked compiler will search for the root of the module
structure of the project:

1. attempt to load `./project.prql`. If successful use it as the root,
2. attempt to load `./mod.prql`. If successful visit parent directory and go
   back to step 1.
3. use compiled file as the root for the module structure.

prql-compiler will provide an interface that has:

- a function that takes a "file loader", which load files on-demand,
- a simple function, that always report "file does not exist" to the compiler,
  and can be used for compiling a single file to a single query.

## Behavior

Any declarations within a module can be referenced from the outside of the
module:

```
module my_module {
    let bangers = (from tracks | take 10)
}

let great_tracks = my_module.bangers
```

If last declaration in a module is a variable declaration that is named `main`,
then the leading `let main = ` can be omitted and expressed only by the
expression itself.

```
module my_module {
    let bangers = (from tracks | take 10)

    from albums | select [title]
}

let album_titles = my_module.main
```

When a file `my_file` is compiled to a query, variable `my_file.main` will be
compiled to RQ.

## Built-in module structure

```
# root module of every project
module project {
	module std {
		func sum a -> ...
		func mean a -> ...
	}

	module default_db {
		# all inferred tables and defined CTEs
	}

	let main = (
		from t = tracks
		select [track_id, title]
	)
}
```
