# Syntax highlighting for Vim

This is a syntax highlighting file for Vim and Neovim.

## Installation

### For Vim

Copy the `prql.vim` file to:

    ~/.vim/syntax/

Then, edit your `~/.vimrc` file and add the following:

```vim
augroup PrqlFileType
  autocmd!
  autocmd BufRead,BufNewFile *.prql setfiletype prql
augroup END
```

### For Neovim

Copy the `prql.vim` file to:

    ~/.config/nvim/syntax/

Then, edit your `~/.config/nvim/init.vim` file.
