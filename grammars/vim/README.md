# Syntax highlighting for Vim

Both Vim and Neovim ship PRQL's syntax file
([`runtime/syntax/prql.vim`](https://github.com/vim/vim/blob/master/runtime/syntax/prql.vim)),
so there's nothing to install from this repo.

## Neovim

Neovim 0.11 and later detect the `.prql` extension and highlight it with no
configuration.

## Vim

Vim needs no configuration on a current version: it detects the `.prql`
extension (since patch 9.0.1319) and bundles the syntax file (since v9.1.1212).

On any Vim older than v9.1.1212, the syntax file isn't bundled — copy
[`prql.vim`](https://github.com/vim/vim/blob/master/runtime/syntax/prql.vim)
into `~/.vim/syntax/`. That's sufficient on 9.0.1319 and later; on a Vim older
than that, the extension isn't detected either, so also set the filetype in your
`~/.vimrc`:

```vim
augroup PrqlFileType
  autocmd!
  autocmd BufRead,BufNewFile *.prql setfiletype prql
augroup END
```
