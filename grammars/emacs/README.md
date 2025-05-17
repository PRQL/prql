# Syntax highlighting for GNU Emacs

This is a syntax highlighting file for GNU Emacs.

## Installation

Copy the `prql-mode.el` file to:

    ~/.emacs.d/custom-modes/

Then, edit your `~/emacs.d/init.el` file and add the following:

```emacs
(add-to-list 'load-path "~/.emacs.d/custom-modes/")
(require 'prql-mode)

(add-to-list 'auto-mode-alist '("\\.prql\\'" . prql-mode))
```
