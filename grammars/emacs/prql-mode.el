;;; prql-mode.el --- Major mode for PRQL language -*- lexical-binding: t;

;; URL: https://github.com/PRQL/prql
;; Keywords: languages
;; Version: 0.1

;;; Commentary:

;; Provides syntax highlighting for PRQL.

;;; Code:

(defvar prql-mode-syntax-table
  (let ((table (make-syntax-table)))
    ;; Define comment syntax
    (modify-syntax-entry ?# "<" table)
    (modify-syntax-entry ?\n ">" table)
    table)
  "Syntax table for `prql-mode'.")

(defconst prql-constants
  '("true" "false" "this" "that" "null")
  "List of PRQL constants.")

(defconst prql-data-types
  '("bool" "float" "int" "int8" "int16" "int32" "int64" "int128" "text" "date" "time" "timestamp")
  "List of PRQL data types.")

(defconst prql-builtin-functions
  '("aggregate" "derive" "filter" "from" "group" "join" "select" "sort" "take" "window")
  "List of PRQL built-in function.")

(defconst prql-operators
  '("!" "=" "&&" "||"
    "+" "-" "*" "/" "%"
    "<" ">" "~=")
  "List of PRQL operators.")

(defconst prql-numbers
  '("0o[0-7]+"
    "0x[0-9a-fA-F]+"
    "0b[01]+"
    "[0-9]+*")
  "Regex for matching PRQL numbers.")

(defconst prql-other-keywords
  '("prql" "case" "let" "type" "alias" "in" "loop" "module")
  "List of other PRQL keywords.")

(defvar prql-font-lock-keywords
  ;; Define keywords and patterns for highlighting
  `(
    ,(cons (regexp-opt prql-constants 'words) 'font-lock-constant-face)
    ,(cons (regexp-opt prql-other-keywords 'words) 'font-lock-keyword-face)
    ,(cons (regexp-opt prql-builtin-functions 'words) 'font-lock-builtin-face)
    ,(cons (regexp-opt prql-data-types 'words) 'font-lock-type-face)
    ,@(mapcar (lambda (kw) (cons kw 'font-lock-builtin-face)) prql-operators)
    ,@(mapcar (lambda (kw) (cons kw 'font-lock-constant-face)) prql-numbers)))

(define-derived-mode prql-mode prog-mode "PRQL"
  "Major mode for editing PRQL code."
  ;; Set the syntax table
  (set-syntax-table prql-mode-syntax-table)
  ;; Set font lock keywords
  (setq font-lock-defaults '((prql-font-lock-keywords)))
  ;; Enable automatic indentation
  (setq indent-tabs-mode nil)
  (setq tab-width 2))

;; Add the mode to the auto-mode-alist for .prql files
(add-to-list 'auto-mode-alist '("\\.prql\\'" . prql-mode))

(provide 'prql-mode)

;;; prql-mode.el ends here
