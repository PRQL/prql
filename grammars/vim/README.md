# Syntax highlighting for Vim

Both Vim and Neovim ship PRQL's syntax file
([`runtime/syntax/prql.vim`](https://github.com/vim/vim/blob/master/runtime/syntax/prql.vim)),
so there's nothing to install from this repo.

## Neovim

Neovim 0.11 and later detect the `.prql` extension and highlight it with no
configuration.

## Vim

Vim bundles the syntax file (since v9.1.1212), but doesn't yet detect the
`.prql` extension, so the filetype has to be set manually. Add the following to
your `~/.vimrc`:

```vim
augroup PrqlFileType
  autocmd!
  autocmd BufRead,BufNewFile *.prql setfiletype prql
augroup END
```

On an older Vim, additionally copy
[`prql.vim`](https://github.com/vim/vim/blob/master/runtime/syntax/prql.vim)
into `~/.vim/syntax/`.
