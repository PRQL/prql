# Grammars / syntax highlighting

PRQL contains multiple grammar definitions to enable tools to highlight PRQL
code. These are all intended to provide as good an experience as the grammar
supports. Please raise any shortcomings in a GitHub issue.

The definitions are somewhat scattered around the codebase; this page serves as
an index.

- [Ace](https://ace.c9.io/) — supported. The grammar is upstream
  ([prql_highlight_rules.js](https://github.com/ajaxorg/ace/blob/master/src/mode/prql_highlight_rules.js)).
  See the [demo](https://prql-lang.org/demos/ace-demo).

- [chroma](https://github.com/alecthomas/chroma) — Go library used by the static
  website generator Hugo. The grammar is upstream
  ([prql.xml](https://github.com/alecthomas/chroma/blob/master/lexers/embedded/prql.xml)).
  See the [demo](https://swapoff.org/chroma/playground/).

- [CotEditor](https://coteditor.com/) — text editor for macOS. File is at
  [`grammars/CotEditor/`](https://github.com/PRQL/prql/tree/main/grammars/CotEditor/).

- [Lezer](https://lezer.codemirror.net/) — used by CodeMirror editors. The PRQL
  file is at
  [`grammars/prql-lezer/README.md`](https://github.com/PRQL/prql/tree/main/grammars/prql-lezer/README.md).

- emacs — used by terminal-based text editor GNU Emacs. File is at
  [`grammars/emacs/`](https://github.com/PRQL/prql/tree/main/grammars/emacs/).

- GtkSourceView — used by GNOME Text Editor, GNOME Builder and other GTK
  applications. File is at
  [`grammars/GtkSourceView/`](https://github.com/PRQL/prql/tree/main/grammars/GtkSourceView/).

- [Handlebars](https://handlebarsjs.com/) — currently duplicated:
  - The book:
    [`book/highlight-prql.js`](https://github.com/PRQL/prql/blob/main/web/book/highlight-prql.js)
  - The website (outside of the book & playground):
    [`website/themes/prql-theme/static/plugins/highlight/prql.js`](https://github.com/PRQL/prql/blob/main/web/book/highlight-prql.js)

- [Helix](https://helix-editor.com/) — supported. The grammar is
  [upstream](https://github.com/helix-editor/helix/tree/master/runtime/queries/prql).

- [Kakoune](https://kakoune.org/) — supported. The grammar is
  [upstream](https://github.com/mawww/kakoune/blob/master/rc/filetype/prql.kak).

- KSyntaxHighlighting — used by Kate, KWrite and KDevelop and other Qt
  applications. File is at
  [`grammars/KSyntaxHighlighting/`](https://github.com/PRQL/prql/tree/main/grammars/KSyntaxHighlighting/).

- [micro](https://micro-editor.github.io/) — used by terminal-based text editor
  Micro. The grammar is
  [upstream](https://github.com/zyedidia/micro/blob/master/runtime/syntax/prql.yaml).

- [nano](https://nano-editor.org/) — used by terminal-based text editor GNU
  nano. File is at
  [`grammars/nano/`](https://github.com/PRQL/prql/tree/main/grammars/nano/).

- Sublime Text — in the [`sublime-prql`](https://github.com/PRQL/sublime-prql/)
  repository.

- TextMate — used by the VS Code extension; in the `prql-vscode` repo in
  [`prql-vscode/syntaxes/prql.tmLanguage.json`](https://github.com/PRQL/prql-vscode/blob/main/syntaxes/prql.tmLanguage.json).

- [Monarch](https://microsoft.github.io/monaco-editor/monarch.html) — used by
  the Monaco editor, which we use for the Playground. The grammar is at
  [`playground/src/workbench/prql-syntax.js`](https://github.com/PRQL/prql/blob/main/web/playground/src/workbench/prql-syntax.js).

- [Pygments](https://pygments.org/) — Python library used by Wikipedia,
  Bitbucket, Sphinx and [more](https://pygments.org/faq/#who-uses-pygments). The
  grammar is upstream
  ([prql.py](https://github.com/pygments/pygments/blob/master/pygments/lexers/prql.py)).
  See the [demo](https://pygments.org/demo/).

- [TEA](https://github.com/psemiletov/tea-qt/) — supported. The grammar is
  [upstream](https://github.com/psemiletov/tea-qt/blob/master/hls/prql.xml).

- [Tree-Sitter](https://tree-sitter.github.io/tree-sitter) — used by the neovim
  and helix. The grammar can be found at
  [https://github.com/PRQL/tree-sitter-prql](https://github.com/PRQL/tree-sitter-prql).

- [vim](https://www.vim.org/) — used by terminal-based text editor Vim.
  Instructions at
  [`grammars/vim/`](https://github.com/PRQL/prql/tree/main/grammars/vim/). The
  grammar is
  [upstream](https://github.com/vim/vim/blob/master/runtime/syntax/prql.vim).
  - [Neovim](https://neovim.io/) supported. Grammar is upstream.
