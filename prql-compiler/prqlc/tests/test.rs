#![cfg(not(target_family = "wasm"))]

use insta_cmd::get_cargo_bin;
use insta_cmd::{assert_cmd_snapshot, StdinCommand};
use std::process::Command;

// Windows has slightly different outputs (e.g. `prqlc.exe` instead of `prqlc`),
// so we exclude.
#[cfg(not(target_family = "windows"))]
#[test]
fn test_help() {
    assert_cmd_snapshot!(Command::new(get_cargo_bin("prqlc")).arg("--help"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    Usage: prqlc [OPTIONS] <COMMAND>

    Commands:
      parse             Parse into PL AST
      fmt               Parse & generate PRQL code back
      annotate          Parse, resolve & combine source with comments annotating relation type
      debug             Parse & resolve, but don't lower into RQ
      resolve           Parse, resolve & lower into RQ
      sql:preprocess    Parse, resolve, lower into RQ & preprocess SRQ
      sql:anchor        Parse, resolve, lower into RQ & preprocess & anchor SRQ
      compile           Parse, resolve, lower into RQ & compile to SQL
      watch             Watch a directory and compile .prql files to .sql files
      list-targets      Show available compile target names
      shell-completion  Print a shell completion for supported shells
      help              Print this message or the help of the given subcommand(s)

    Options:
          --color <WHEN>  Controls when to use color [default: auto] [possible values: auto, always,
                          never]
      -h, --help          Print help
      -V, --version       Print version

    ----- stderr -----
    "###);
}

#[test]
fn test_get_targets() {
    assert_cmd_snapshot!(Command::new(get_cargo_bin("prqlc")).arg("list-targets"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    sql.any
    sql.ansi
    sql.bigquery
    sql.clickhouse
    sql.duckdb
    sql.generic
    sql.hive
    sql.mssql
    sql.mysql
    sql.postgres
    sql.sqlite
    sql.snowflake

    ----- stderr -----
    "###);
}

#[test]
fn test_compile() {
    let mut cmd = StdinCommand::new(get_cargo_bin("prqlc"), "from tracks");

    // TODO: fix
    assert_cmd_snapshot!(cmd.arg("compile"), @r###"
    success: false
    exit_code: 1
    ----- stdout -----
    [E0001] Error: Missing main pipeline

    ----- stderr -----
    "###);
}

#[test]
fn test_shell_completion() {
    assert_cmd_snapshot!(Command::new(get_cargo_bin("prqlc")).arg("shell-completion").arg("bash"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    _prqlc() {
        local i cur prev opts cmd
        COMPREPLY=()
        cur="${COMP_WORDS[COMP_CWORD]}"
        prev="${COMP_WORDS[COMP_CWORD-1]}"
        cmd=""
        opts=""

        for i in ${COMP_WORDS[@]}
        do
            case "${cmd},${i}" in
                ",$1")
                    cmd="prqlc"
                    ;;
                prqlc,annotate)
                    cmd="prqlc__annotate"
                    ;;
                prqlc,compile)
                    cmd="prqlc__compile"
                    ;;
                prqlc,debug)
                    cmd="prqlc__debug"
                    ;;
                prqlc,fmt)
                    cmd="prqlc__fmt"
                    ;;
                prqlc,help)
                    cmd="prqlc__help"
                    ;;
                prqlc,list-targets)
                    cmd="prqlc__list__targets"
                    ;;
                prqlc,parse)
                    cmd="prqlc__parse"
                    ;;
                prqlc,resolve)
                    cmd="prqlc__resolve"
                    ;;
                prqlc,shell-completion)
                    cmd="prqlc__shell__completion"
                    ;;
                prqlc,sql:anchor)
                    cmd="prqlc__sql:anchor"
                    ;;
                prqlc,sql:preprocess)
                    cmd="prqlc__sql:preprocess"
                    ;;
                prqlc,watch)
                    cmd="prqlc__watch"
                    ;;
                prqlc__help,annotate)
                    cmd="prqlc__help__annotate"
                    ;;
                prqlc__help,compile)
                    cmd="prqlc__help__compile"
                    ;;
                prqlc__help,debug)
                    cmd="prqlc__help__debug"
                    ;;
                prqlc__help,fmt)
                    cmd="prqlc__help__fmt"
                    ;;
                prqlc__help,help)
                    cmd="prqlc__help__help"
                    ;;
                prqlc__help,list-targets)
                    cmd="prqlc__help__list__targets"
                    ;;
                prqlc__help,parse)
                    cmd="prqlc__help__parse"
                    ;;
                prqlc__help,resolve)
                    cmd="prqlc__help__resolve"
                    ;;
                prqlc__help,shell-completion)
                    cmd="prqlc__help__shell__completion"
                    ;;
                prqlc__help,sql:anchor)
                    cmd="prqlc__help__sql:anchor"
                    ;;
                prqlc__help,sql:preprocess)
                    cmd="prqlc__help__sql:preprocess"
                    ;;
                prqlc__help,watch)
                    cmd="prqlc__help__watch"
                    ;;
                *)
                    ;;
            esac
        done

        case "${cmd}" in
            prqlc)
                opts="-h -V --color --help --version parse fmt annotate debug resolve sql:preprocess sql:anchor compile watch list-targets shell-completion help"
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    --color)
                        COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                        return 0
                        ;;
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__annotate)
                opts="-h --color --help [INPUT] [OUTPUT] [MAIN_PATH]"
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    --color)
                        COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                        return 0
                        ;;
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__compile)
                opts="-t -h --include-signature-comment --target --color --help [INPUT] [OUTPUT] [MAIN_PATH]"
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    --target)
                        COMPREPLY=($(compgen -f "${cur}"))
                        return 0
                        ;;
                    -t)
                        COMPREPLY=($(compgen -f "${cur}"))
                        return 0
                        ;;
                    --color)
                        COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                        return 0
                        ;;
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__debug)
                opts="-h --color --help [INPUT] [OUTPUT] [MAIN_PATH]"
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    --color)
                        COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                        return 0
                        ;;
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__fmt)
                opts="-h --color --help [INPUT] [OUTPUT] [MAIN_PATH]"
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    --color)
                        COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                        return 0
                        ;;
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__help)
                opts="parse fmt annotate debug resolve sql:preprocess sql:anchor compile watch list-targets shell-completion help"
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__help__annotate)
                opts=""
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__help__compile)
                opts=""
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__help__debug)
                opts=""
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__help__fmt)
                opts=""
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__help__help)
                opts=""
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__help__list__targets)
                opts=""
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__help__parse)
                opts=""
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__help__resolve)
                opts=""
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__help__shell__completion)
                opts=""
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__help__sql:anchor)
                opts=""
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__help__sql:preprocess)
                opts=""
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__help__watch)
                opts=""
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 3 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__list__targets)
                opts="-h --color --help"
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    --color)
                        COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                        return 0
                        ;;
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__parse)
                opts="-h --format --color --help [INPUT] [OUTPUT] [MAIN_PATH]"
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    --format)
                        COMPREPLY=($(compgen -W "json yaml" -- "${cur}"))
                        return 0
                        ;;
                    --color)
                        COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                        return 0
                        ;;
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__resolve)
                opts="-h --format --color --help [INPUT] [OUTPUT] [MAIN_PATH]"
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    --format)
                        COMPREPLY=($(compgen -W "json yaml" -- "${cur}"))
                        return 0
                        ;;
                    --color)
                        COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                        return 0
                        ;;
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__shell__completion)
                opts="-h --color --help bash elvish fig fish nushell powershell zsh"
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    --color)
                        COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                        return 0
                        ;;
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__sql:anchor)
                opts="-h --format --color --help [INPUT] [OUTPUT] [MAIN_PATH]"
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    --format)
                        COMPREPLY=($(compgen -W "json yaml" -- "${cur}"))
                        return 0
                        ;;
                    --color)
                        COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                        return 0
                        ;;
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__sql:preprocess)
                opts="-h --color --help [INPUT] [OUTPUT] [MAIN_PATH]"
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    --color)
                        COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                        return 0
                        ;;
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
            prqlc__watch)
                opts="-h --no-format --no-signature --color --help <PATH>"
                if [[ ${cur} == -* || ${COMP_CWORD} -eq 2 ]] ; then
                    COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                    return 0
                fi
                case "${prev}" in
                    --color)
                        COMPREPLY=($(compgen -W "auto always never" -- "${cur}"))
                        return 0
                        ;;
                    *)
                        COMPREPLY=()
                        ;;
                esac
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
                ;;
        esac
    }

    complete -F _prqlc -o bashdefault -o default prqlc

    ----- stderr -----
    "###);
    assert_cmd_snapshot!(Command::new(get_cargo_bin("prqlc")).arg("shell-completion").arg("zsh"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    #compdef prqlc

    autoload -U is-at-least

    _prqlc() {
        typeset -A opt_args
        typeset -a _arguments_options
        local ret=1

        if is-at-least 5.2; then
            _arguments_options=(-s -S -C)
        else
            _arguments_options=(-s -C)
        fi

        local context curcontext="$curcontext" state line
        _arguments "${_arguments_options[@]}" \
    '--color=[Controls when to use color]:WHEN:(auto always never)' \
    '-h[Print help]' \
    '--help[Print help]' \
    '-V[Print version]' \
    '--version[Print version]' \
    ":: :_prqlc_commands" \
    "*::: :->prqlc" \
    && ret=0
        case $state in
        (prqlc)
            words=($line[1] "${words[@]}")
            (( CURRENT += 1 ))
            curcontext="${curcontext%:*:*}:prqlc-command-$line[1]:"
            case $line[1] in
                (parse)
    _arguments "${_arguments_options[@]}" \
    '--format=[]:FORMAT:(json yaml)' \
    '--color=[Controls when to use color]:WHEN:(auto always never)' \
    '-h[Print help]' \
    '--help[Print help]' \
    '::input:_files' \
    '::output:_files' \
    '::main_path -- Identifier of the main pipeline:_files' \
    && ret=0
    ;;
    (fmt)
    _arguments "${_arguments_options[@]}" \
    '--color=[Controls when to use color]:WHEN:(auto always never)' \
    '-h[Print help]' \
    '--help[Print help]' \
    '::input:_files' \
    '::output:_files' \
    '::main_path -- Identifier of the main pipeline:_files' \
    && ret=0
    ;;
    (annotate)
    _arguments "${_arguments_options[@]}" \
    '--color=[Controls when to use color]:WHEN:(auto always never)' \
    '-h[Print help]' \
    '--help[Print help]' \
    '::input:_files' \
    '::output:_files' \
    '::main_path -- Identifier of the main pipeline:_files' \
    && ret=0
    ;;
    (debug)
    _arguments "${_arguments_options[@]}" \
    '--color=[Controls when to use color]:WHEN:(auto always never)' \
    '-h[Print help]' \
    '--help[Print help]' \
    '::input:_files' \
    '::output:_files' \
    '::main_path -- Identifier of the main pipeline:_files' \
    && ret=0
    ;;
    (resolve)
    _arguments "${_arguments_options[@]}" \
    '--format=[]:FORMAT:(json yaml)' \
    '--color=[Controls when to use color]:WHEN:(auto always never)' \
    '-h[Print help]' \
    '--help[Print help]' \
    '::input:_files' \
    '::output:_files' \
    '::main_path -- Identifier of the main pipeline:_files' \
    && ret=0
    ;;
    (sql:preprocess)
    _arguments "${_arguments_options[@]}" \
    '--color=[Controls when to use color]:WHEN:(auto always never)' \
    '-h[Print help]' \
    '--help[Print help]' \
    '::input:_files' \
    '::output:_files' \
    '::main_path -- Identifier of the main pipeline:_files' \
    && ret=0
    ;;
    (sql:anchor)
    _arguments "${_arguments_options[@]}" \
    '--format=[]:FORMAT:(json yaml)' \
    '--color=[Controls when to use color]:WHEN:(auto always never)' \
    '-h[Print help (see more with '\''--help'\'')]' \
    '--help[Print help (see more with '\''--help'\'')]' \
    '::input:_files' \
    '::output:_files' \
    '::main_path -- Identifier of the main pipeline:_files' \
    && ret=0
    ;;
    (compile)
    _arguments "${_arguments_options[@]}" \
    '-t+[]:TARGET: ' \
    '--target=[]:TARGET: ' \
    '--color=[Controls when to use color]:WHEN:(auto always never)' \
    '--include-signature-comment[]' \
    '-h[Print help (see more with '\''--help'\'')]' \
    '--help[Print help (see more with '\''--help'\'')]' \
    '::input:_files' \
    '::output:_files' \
    '::main_path -- Identifier of the main pipeline:_files' \
    && ret=0
    ;;
    (watch)
    _arguments "${_arguments_options[@]}" \
    '--color=[Controls when to use color]:WHEN:(auto always never)' \
    '--no-format[]' \
    '--no-signature[]' \
    '-h[Print help]' \
    '--help[Print help]' \
    ':path -- Directory or file to watch for changes:' \
    && ret=0
    ;;
    (list-targets)
    _arguments "${_arguments_options[@]}" \
    '--color=[Controls when to use color]:WHEN:(auto always never)' \
    '-h[Print help]' \
    '--help[Print help]' \
    && ret=0
    ;;
    (shell-completion)
    _arguments "${_arguments_options[@]}" \
    '--color=[Controls when to use color]:WHEN:(auto always never)' \
    '-h[Print help]' \
    '--help[Print help]' \
    ':shell:(bash elvish fig fish nushell powershell zsh)' \
    && ret=0
    ;;
    (help)
    _arguments "${_arguments_options[@]}" \
    ":: :_prqlc__help_commands" \
    "*::: :->help" \
    && ret=0

        case $state in
        (help)
            words=($line[1] "${words[@]}")
            (( CURRENT += 1 ))
            curcontext="${curcontext%:*:*}:prqlc-help-command-$line[1]:"
            case $line[1] in
                (parse)
    _arguments "${_arguments_options[@]}" \
    && ret=0
    ;;
    (fmt)
    _arguments "${_arguments_options[@]}" \
    && ret=0
    ;;
    (annotate)
    _arguments "${_arguments_options[@]}" \
    && ret=0
    ;;
    (debug)
    _arguments "${_arguments_options[@]}" \
    && ret=0
    ;;
    (resolve)
    _arguments "${_arguments_options[@]}" \
    && ret=0
    ;;
    (sql:preprocess)
    _arguments "${_arguments_options[@]}" \
    && ret=0
    ;;
    (sql:anchor)
    _arguments "${_arguments_options[@]}" \
    && ret=0
    ;;
    (compile)
    _arguments "${_arguments_options[@]}" \
    && ret=0
    ;;
    (watch)
    _arguments "${_arguments_options[@]}" \
    && ret=0
    ;;
    (list-targets)
    _arguments "${_arguments_options[@]}" \
    && ret=0
    ;;
    (shell-completion)
    _arguments "${_arguments_options[@]}" \
    && ret=0
    ;;
    (help)
    _arguments "${_arguments_options[@]}" \
    && ret=0
    ;;
            esac
        ;;
    esac
    ;;
            esac
        ;;
    esac
    }

    (( $+functions[_prqlc_commands] )) ||
    _prqlc_commands() {
        local commands; commands=(
    'parse:Parse into PL AST' \
    'fmt:Parse & generate PRQL code back' \
    'annotate:Parse, resolve & combine source with comments annotating relation type' \
    'debug:Parse & resolve, but don'\''t lower into RQ' \
    'resolve:Parse, resolve & lower into RQ' \
    'sql:preprocess:Parse, resolve, lower into RQ & preprocess SRQ' \
    'sql:anchor:Parse, resolve, lower into RQ & preprocess & anchor SRQ' \
    'compile:Parse, resolve, lower into RQ & compile to SQL' \
    'watch:Watch a directory and compile .prql files to .sql files' \
    'list-targets:Show available compile target names' \
    'shell-completion:Print a shell completion for supported shells' \
    'help:Print this message or the help of the given subcommand(s)' \
        )
        _describe -t commands 'prqlc commands' commands "$@"
    }
    (( $+functions[_prqlc__annotate_commands] )) ||
    _prqlc__annotate_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc annotate commands' commands "$@"
    }
    (( $+functions[_prqlc__help__annotate_commands] )) ||
    _prqlc__help__annotate_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc help annotate commands' commands "$@"
    }
    (( $+functions[_prqlc__compile_commands] )) ||
    _prqlc__compile_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc compile commands' commands "$@"
    }
    (( $+functions[_prqlc__help__compile_commands] )) ||
    _prqlc__help__compile_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc help compile commands' commands "$@"
    }
    (( $+functions[_prqlc__debug_commands] )) ||
    _prqlc__debug_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc debug commands' commands "$@"
    }
    (( $+functions[_prqlc__help__debug_commands] )) ||
    _prqlc__help__debug_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc help debug commands' commands "$@"
    }
    (( $+functions[_prqlc__fmt_commands] )) ||
    _prqlc__fmt_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc fmt commands' commands "$@"
    }
    (( $+functions[_prqlc__help__fmt_commands] )) ||
    _prqlc__help__fmt_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc help fmt commands' commands "$@"
    }
    (( $+functions[_prqlc__help_commands] )) ||
    _prqlc__help_commands() {
        local commands; commands=(
    'parse:Parse into PL AST' \
    'fmt:Parse & generate PRQL code back' \
    'annotate:Parse, resolve & combine source with comments annotating relation type' \
    'debug:Parse & resolve, but don'\''t lower into RQ' \
    'resolve:Parse, resolve & lower into RQ' \
    'sql:preprocess:Parse, resolve, lower into RQ & preprocess SRQ' \
    'sql:anchor:Parse, resolve, lower into RQ & preprocess & anchor SRQ' \
    'compile:Parse, resolve, lower into RQ & compile to SQL' \
    'watch:Watch a directory and compile .prql files to .sql files' \
    'list-targets:Show available compile target names' \
    'shell-completion:Print a shell completion for supported shells' \
    'help:Print this message or the help of the given subcommand(s)' \
        )
        _describe -t commands 'prqlc help commands' commands "$@"
    }
    (( $+functions[_prqlc__help__help_commands] )) ||
    _prqlc__help__help_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc help help commands' commands "$@"
    }
    (( $+functions[_prqlc__help__list-targets_commands] )) ||
    _prqlc__help__list-targets_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc help list-targets commands' commands "$@"
    }
    (( $+functions[_prqlc__list-targets_commands] )) ||
    _prqlc__list-targets_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc list-targets commands' commands "$@"
    }
    (( $+functions[_prqlc__help__parse_commands] )) ||
    _prqlc__help__parse_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc help parse commands' commands "$@"
    }
    (( $+functions[_prqlc__parse_commands] )) ||
    _prqlc__parse_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc parse commands' commands "$@"
    }
    (( $+functions[_prqlc__help__resolve_commands] )) ||
    _prqlc__help__resolve_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc help resolve commands' commands "$@"
    }
    (( $+functions[_prqlc__resolve_commands] )) ||
    _prqlc__resolve_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc resolve commands' commands "$@"
    }
    (( $+functions[_prqlc__help__shell-completion_commands] )) ||
    _prqlc__help__shell-completion_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc help shell-completion commands' commands "$@"
    }
    (( $+functions[_prqlc__shell-completion_commands] )) ||
    _prqlc__shell-completion_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc shell-completion commands' commands "$@"
    }
    (( $+functions[_prqlc__help__sql:anchor_commands] )) ||
    _prqlc__help__sql:anchor_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc help sql:anchor commands' commands "$@"
    }
    (( $+functions[_prqlc__sql:anchor_commands] )) ||
    _prqlc__sql:anchor_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc sql:anchor commands' commands "$@"
    }
    (( $+functions[_prqlc__help__sql:preprocess_commands] )) ||
    _prqlc__help__sql:preprocess_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc help sql:preprocess commands' commands "$@"
    }
    (( $+functions[_prqlc__sql:preprocess_commands] )) ||
    _prqlc__sql:preprocess_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc sql:preprocess commands' commands "$@"
    }
    (( $+functions[_prqlc__help__watch_commands] )) ||
    _prqlc__help__watch_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc help watch commands' commands "$@"
    }
    (( $+functions[_prqlc__watch_commands] )) ||
    _prqlc__watch_commands() {
        local commands; commands=()
        _describe -t commands 'prqlc watch commands' commands "$@"
    }

    if [ "$funcstack[1]" = "_prqlc" ]; then
        _prqlc "$@"
    else
        compdef _prqlc prqlc
    fi

    ----- stderr -----
    "###);
    assert_cmd_snapshot!(Command::new(get_cargo_bin("prqlc")).arg("shell-completion").arg("fish"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----
    complete -c prqlc -n "__fish_use_subcommand" -l color -d 'Controls when to use color' -r -f -a "{auto	,always	,never	}"
    complete -c prqlc -n "__fish_use_subcommand" -s h -l help -d 'Print help'
    complete -c prqlc -n "__fish_use_subcommand" -s V -l version -d 'Print version'
    complete -c prqlc -n "__fish_use_subcommand" -f -a "parse" -d 'Parse into PL AST'
    complete -c prqlc -n "__fish_use_subcommand" -f -a "fmt" -d 'Parse & generate PRQL code back'
    complete -c prqlc -n "__fish_use_subcommand" -f -a "annotate" -d 'Parse, resolve & combine source with comments annotating relation type'
    complete -c prqlc -n "__fish_use_subcommand" -f -a "debug" -d 'Parse & resolve, but don\'t lower into RQ'
    complete -c prqlc -n "__fish_use_subcommand" -f -a "resolve" -d 'Parse, resolve & lower into RQ'
    complete -c prqlc -n "__fish_use_subcommand" -f -a "sql:preprocess" -d 'Parse, resolve, lower into RQ & preprocess SRQ'
    complete -c prqlc -n "__fish_use_subcommand" -f -a "sql:anchor" -d 'Parse, resolve, lower into RQ & preprocess & anchor SRQ'
    complete -c prqlc -n "__fish_use_subcommand" -f -a "compile" -d 'Parse, resolve, lower into RQ & compile to SQL'
    complete -c prqlc -n "__fish_use_subcommand" -f -a "watch" -d 'Watch a directory and compile .prql files to .sql files'
    complete -c prqlc -n "__fish_use_subcommand" -f -a "list-targets" -d 'Show available compile target names'
    complete -c prqlc -n "__fish_use_subcommand" -f -a "shell-completion" -d 'Print a shell completion for supported shells'
    complete -c prqlc -n "__fish_use_subcommand" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
    complete -c prqlc -n "__fish_seen_subcommand_from parse" -l format -r -f -a "{json	,yaml	}"
    complete -c prqlc -n "__fish_seen_subcommand_from parse" -l color -d 'Controls when to use color' -r -f -a "{auto	,always	,never	}"
    complete -c prqlc -n "__fish_seen_subcommand_from parse" -s h -l help -d 'Print help'
    complete -c prqlc -n "__fish_seen_subcommand_from fmt" -l color -d 'Controls when to use color' -r -f -a "{auto	,always	,never	}"
    complete -c prqlc -n "__fish_seen_subcommand_from fmt" -s h -l help -d 'Print help'
    complete -c prqlc -n "__fish_seen_subcommand_from annotate" -l color -d 'Controls when to use color' -r -f -a "{auto	,always	,never	}"
    complete -c prqlc -n "__fish_seen_subcommand_from annotate" -s h -l help -d 'Print help'
    complete -c prqlc -n "__fish_seen_subcommand_from debug" -l color -d 'Controls when to use color' -r -f -a "{auto	,always	,never	}"
    complete -c prqlc -n "__fish_seen_subcommand_from debug" -s h -l help -d 'Print help'
    complete -c prqlc -n "__fish_seen_subcommand_from resolve" -l format -r -f -a "{json	,yaml	}"
    complete -c prqlc -n "__fish_seen_subcommand_from resolve" -l color -d 'Controls when to use color' -r -f -a "{auto	,always	,never	}"
    complete -c prqlc -n "__fish_seen_subcommand_from resolve" -s h -l help -d 'Print help'
    complete -c prqlc -n "__fish_seen_subcommand_from sql:preprocess" -l color -d 'Controls when to use color' -r -f -a "{auto	,always	,never	}"
    complete -c prqlc -n "__fish_seen_subcommand_from sql:preprocess" -s h -l help -d 'Print help'
    complete -c prqlc -n "__fish_seen_subcommand_from sql:anchor" -l format -r -f -a "{json	,yaml	}"
    complete -c prqlc -n "__fish_seen_subcommand_from sql:anchor" -l color -d 'Controls when to use color' -r -f -a "{auto	,always	,never	}"
    complete -c prqlc -n "__fish_seen_subcommand_from sql:anchor" -s h -l help -d 'Print help (see more with \'--help\')'
    complete -c prqlc -n "__fish_seen_subcommand_from compile" -s t -l target -r
    complete -c prqlc -n "__fish_seen_subcommand_from compile" -l color -d 'Controls when to use color' -r -f -a "{auto	,always	,never	}"
    complete -c prqlc -n "__fish_seen_subcommand_from compile" -l include-signature-comment
    complete -c prqlc -n "__fish_seen_subcommand_from compile" -s h -l help -d 'Print help (see more with \'--help\')'
    complete -c prqlc -n "__fish_seen_subcommand_from watch" -l color -d 'Controls when to use color' -r -f -a "{auto	,always	,never	}"
    complete -c prqlc -n "__fish_seen_subcommand_from watch" -l no-format
    complete -c prqlc -n "__fish_seen_subcommand_from watch" -l no-signature
    complete -c prqlc -n "__fish_seen_subcommand_from watch" -s h -l help -d 'Print help'
    complete -c prqlc -n "__fish_seen_subcommand_from list-targets" -l color -d 'Controls when to use color' -r -f -a "{auto	,always	,never	}"
    complete -c prqlc -n "__fish_seen_subcommand_from list-targets" -s h -l help -d 'Print help'
    complete -c prqlc -n "__fish_seen_subcommand_from shell-completion" -l color -d 'Controls when to use color' -r -f -a "{auto	,always	,never	}"
    complete -c prqlc -n "__fish_seen_subcommand_from shell-completion" -s h -l help -d 'Print help'
    complete -c prqlc -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from parse; and not __fish_seen_subcommand_from fmt; and not __fish_seen_subcommand_from annotate; and not __fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from resolve; and not __fish_seen_subcommand_from sql:preprocess; and not __fish_seen_subcommand_from sql:anchor; and not __fish_seen_subcommand_from compile; and not __fish_seen_subcommand_from watch; and not __fish_seen_subcommand_from list-targets; and not __fish_seen_subcommand_from shell-completion; and not __fish_seen_subcommand_from help" -f -a "parse" -d 'Parse into PL AST'
    complete -c prqlc -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from parse; and not __fish_seen_subcommand_from fmt; and not __fish_seen_subcommand_from annotate; and not __fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from resolve; and not __fish_seen_subcommand_from sql:preprocess; and not __fish_seen_subcommand_from sql:anchor; and not __fish_seen_subcommand_from compile; and not __fish_seen_subcommand_from watch; and not __fish_seen_subcommand_from list-targets; and not __fish_seen_subcommand_from shell-completion; and not __fish_seen_subcommand_from help" -f -a "fmt" -d 'Parse & generate PRQL code back'
    complete -c prqlc -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from parse; and not __fish_seen_subcommand_from fmt; and not __fish_seen_subcommand_from annotate; and not __fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from resolve; and not __fish_seen_subcommand_from sql:preprocess; and not __fish_seen_subcommand_from sql:anchor; and not __fish_seen_subcommand_from compile; and not __fish_seen_subcommand_from watch; and not __fish_seen_subcommand_from list-targets; and not __fish_seen_subcommand_from shell-completion; and not __fish_seen_subcommand_from help" -f -a "annotate" -d 'Parse, resolve & combine source with comments annotating relation type'
    complete -c prqlc -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from parse; and not __fish_seen_subcommand_from fmt; and not __fish_seen_subcommand_from annotate; and not __fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from resolve; and not __fish_seen_subcommand_from sql:preprocess; and not __fish_seen_subcommand_from sql:anchor; and not __fish_seen_subcommand_from compile; and not __fish_seen_subcommand_from watch; and not __fish_seen_subcommand_from list-targets; and not __fish_seen_subcommand_from shell-completion; and not __fish_seen_subcommand_from help" -f -a "debug" -d 'Parse & resolve, but don\'t lower into RQ'
    complete -c prqlc -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from parse; and not __fish_seen_subcommand_from fmt; and not __fish_seen_subcommand_from annotate; and not __fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from resolve; and not __fish_seen_subcommand_from sql:preprocess; and not __fish_seen_subcommand_from sql:anchor; and not __fish_seen_subcommand_from compile; and not __fish_seen_subcommand_from watch; and not __fish_seen_subcommand_from list-targets; and not __fish_seen_subcommand_from shell-completion; and not __fish_seen_subcommand_from help" -f -a "resolve" -d 'Parse, resolve & lower into RQ'
    complete -c prqlc -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from parse; and not __fish_seen_subcommand_from fmt; and not __fish_seen_subcommand_from annotate; and not __fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from resolve; and not __fish_seen_subcommand_from sql:preprocess; and not __fish_seen_subcommand_from sql:anchor; and not __fish_seen_subcommand_from compile; and not __fish_seen_subcommand_from watch; and not __fish_seen_subcommand_from list-targets; and not __fish_seen_subcommand_from shell-completion; and not __fish_seen_subcommand_from help" -f -a "sql:preprocess" -d 'Parse, resolve, lower into RQ & preprocess SRQ'
    complete -c prqlc -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from parse; and not __fish_seen_subcommand_from fmt; and not __fish_seen_subcommand_from annotate; and not __fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from resolve; and not __fish_seen_subcommand_from sql:preprocess; and not __fish_seen_subcommand_from sql:anchor; and not __fish_seen_subcommand_from compile; and not __fish_seen_subcommand_from watch; and not __fish_seen_subcommand_from list-targets; and not __fish_seen_subcommand_from shell-completion; and not __fish_seen_subcommand_from help" -f -a "sql:anchor" -d 'Parse, resolve, lower into RQ & preprocess & anchor SRQ'
    complete -c prqlc -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from parse; and not __fish_seen_subcommand_from fmt; and not __fish_seen_subcommand_from annotate; and not __fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from resolve; and not __fish_seen_subcommand_from sql:preprocess; and not __fish_seen_subcommand_from sql:anchor; and not __fish_seen_subcommand_from compile; and not __fish_seen_subcommand_from watch; and not __fish_seen_subcommand_from list-targets; and not __fish_seen_subcommand_from shell-completion; and not __fish_seen_subcommand_from help" -f -a "compile" -d 'Parse, resolve, lower into RQ & compile to SQL'
    complete -c prqlc -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from parse; and not __fish_seen_subcommand_from fmt; and not __fish_seen_subcommand_from annotate; and not __fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from resolve; and not __fish_seen_subcommand_from sql:preprocess; and not __fish_seen_subcommand_from sql:anchor; and not __fish_seen_subcommand_from compile; and not __fish_seen_subcommand_from watch; and not __fish_seen_subcommand_from list-targets; and not __fish_seen_subcommand_from shell-completion; and not __fish_seen_subcommand_from help" -f -a "watch" -d 'Watch a directory and compile .prql files to .sql files'
    complete -c prqlc -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from parse; and not __fish_seen_subcommand_from fmt; and not __fish_seen_subcommand_from annotate; and not __fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from resolve; and not __fish_seen_subcommand_from sql:preprocess; and not __fish_seen_subcommand_from sql:anchor; and not __fish_seen_subcommand_from compile; and not __fish_seen_subcommand_from watch; and not __fish_seen_subcommand_from list-targets; and not __fish_seen_subcommand_from shell-completion; and not __fish_seen_subcommand_from help" -f -a "list-targets" -d 'Show available compile target names'
    complete -c prqlc -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from parse; and not __fish_seen_subcommand_from fmt; and not __fish_seen_subcommand_from annotate; and not __fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from resolve; and not __fish_seen_subcommand_from sql:preprocess; and not __fish_seen_subcommand_from sql:anchor; and not __fish_seen_subcommand_from compile; and not __fish_seen_subcommand_from watch; and not __fish_seen_subcommand_from list-targets; and not __fish_seen_subcommand_from shell-completion; and not __fish_seen_subcommand_from help" -f -a "shell-completion" -d 'Print a shell completion for supported shells'
    complete -c prqlc -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from parse; and not __fish_seen_subcommand_from fmt; and not __fish_seen_subcommand_from annotate; and not __fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from resolve; and not __fish_seen_subcommand_from sql:preprocess; and not __fish_seen_subcommand_from sql:anchor; and not __fish_seen_subcommand_from compile; and not __fish_seen_subcommand_from watch; and not __fish_seen_subcommand_from list-targets; and not __fish_seen_subcommand_from shell-completion; and not __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'

    ----- stderr -----
    "###);
    assert_cmd_snapshot!(Command::new(get_cargo_bin("prqlc")).arg("shell-completion").arg("powershell"), @r###"
    success: true
    exit_code: 0
    ----- stdout -----

    using namespace System.Management.Automation
    using namespace System.Management.Automation.Language

    Register-ArgumentCompleter -Native -CommandName 'prqlc' -ScriptBlock {
        param($wordToComplete, $commandAst, $cursorPosition)

        $commandElements = $commandAst.CommandElements
        $command = @(
            'prqlc'
            for ($i = 1; $i -lt $commandElements.Count; $i++) {
                $element = $commandElements[$i]
                if ($element -isnot [StringConstantExpressionAst] -or
                    $element.StringConstantType -ne [StringConstantType]::BareWord -or
                    $element.Value.StartsWith('-') -or
                    $element.Value -eq $wordToComplete) {
                    break
            }
            $element.Value
        }) -join ';'

        $completions = @(switch ($command) {
            'prqlc' {
                [CompletionResult]::new('--color', 'color', [CompletionResultType]::ParameterName, 'Controls when to use color')
                [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
                [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
                [CompletionResult]::new('-V', 'V', [CompletionResultType]::ParameterName, 'Print version')
                [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Print version')
                [CompletionResult]::new('parse', 'parse', [CompletionResultType]::ParameterValue, 'Parse into PL AST')
                [CompletionResult]::new('fmt', 'fmt', [CompletionResultType]::ParameterValue, 'Parse & generate PRQL code back')
                [CompletionResult]::new('annotate', 'annotate', [CompletionResultType]::ParameterValue, 'Parse, resolve & combine source with comments annotating relation type')
                [CompletionResult]::new('debug', 'debug', [CompletionResultType]::ParameterValue, 'Parse & resolve, but don''t lower into RQ')
                [CompletionResult]::new('resolve', 'resolve', [CompletionResultType]::ParameterValue, 'Parse, resolve & lower into RQ')
                [CompletionResult]::new('sql:preprocess', 'sql:preprocess', [CompletionResultType]::ParameterValue, 'Parse, resolve, lower into RQ & preprocess SRQ')
                [CompletionResult]::new('sql:anchor', 'sql:anchor', [CompletionResultType]::ParameterValue, 'Parse, resolve, lower into RQ & preprocess & anchor SRQ')
                [CompletionResult]::new('compile', 'compile', [CompletionResultType]::ParameterValue, 'Parse, resolve, lower into RQ & compile to SQL')
                [CompletionResult]::new('watch', 'watch', [CompletionResultType]::ParameterValue, 'Watch a directory and compile .prql files to .sql files')
                [CompletionResult]::new('list-targets', 'list-targets', [CompletionResultType]::ParameterValue, 'Show available compile target names')
                [CompletionResult]::new('shell-completion', 'shell-completion', [CompletionResultType]::ParameterValue, 'Print a shell completion for supported shells')
                [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
                break
            }
            'prqlc;parse' {
                [CompletionResult]::new('--format', 'format', [CompletionResultType]::ParameterName, 'format')
                [CompletionResult]::new('--color', 'color', [CompletionResultType]::ParameterName, 'Controls when to use color')
                [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
                [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
                break
            }
            'prqlc;fmt' {
                [CompletionResult]::new('--color', 'color', [CompletionResultType]::ParameterName, 'Controls when to use color')
                [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
                [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
                break
            }
            'prqlc;annotate' {
                [CompletionResult]::new('--color', 'color', [CompletionResultType]::ParameterName, 'Controls when to use color')
                [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
                [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
                break
            }
            'prqlc;debug' {
                [CompletionResult]::new('--color', 'color', [CompletionResultType]::ParameterName, 'Controls when to use color')
                [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
                [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
                break
            }
            'prqlc;resolve' {
                [CompletionResult]::new('--format', 'format', [CompletionResultType]::ParameterName, 'format')
                [CompletionResult]::new('--color', 'color', [CompletionResultType]::ParameterName, 'Controls when to use color')
                [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
                [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
                break
            }
            'prqlc;sql:preprocess' {
                [CompletionResult]::new('--color', 'color', [CompletionResultType]::ParameterName, 'Controls when to use color')
                [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
                [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
                break
            }
            'prqlc;sql:anchor' {
                [CompletionResult]::new('--format', 'format', [CompletionResultType]::ParameterName, 'format')
                [CompletionResult]::new('--color', 'color', [CompletionResultType]::ParameterName, 'Controls when to use color')
                [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
                [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
                break
            }
            'prqlc;compile' {
                [CompletionResult]::new('-t', 't', [CompletionResultType]::ParameterName, 't')
                [CompletionResult]::new('--target', 'target', [CompletionResultType]::ParameterName, 'target')
                [CompletionResult]::new('--color', 'color', [CompletionResultType]::ParameterName, 'Controls when to use color')
                [CompletionResult]::new('--include-signature-comment', 'include-signature-comment', [CompletionResultType]::ParameterName, 'include-signature-comment')
                [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
                [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
                break
            }
            'prqlc;watch' {
                [CompletionResult]::new('--color', 'color', [CompletionResultType]::ParameterName, 'Controls when to use color')
                [CompletionResult]::new('--no-format', 'no-format', [CompletionResultType]::ParameterName, 'no-format')
                [CompletionResult]::new('--no-signature', 'no-signature', [CompletionResultType]::ParameterName, 'no-signature')
                [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
                [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
                break
            }
            'prqlc;list-targets' {
                [CompletionResult]::new('--color', 'color', [CompletionResultType]::ParameterName, 'Controls when to use color')
                [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
                [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
                break
            }
            'prqlc;shell-completion' {
                [CompletionResult]::new('--color', 'color', [CompletionResultType]::ParameterName, 'Controls when to use color')
                [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
                [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
                break
            }
            'prqlc;help' {
                [CompletionResult]::new('parse', 'parse', [CompletionResultType]::ParameterValue, 'Parse into PL AST')
                [CompletionResult]::new('fmt', 'fmt', [CompletionResultType]::ParameterValue, 'Parse & generate PRQL code back')
                [CompletionResult]::new('annotate', 'annotate', [CompletionResultType]::ParameterValue, 'Parse, resolve & combine source with comments annotating relation type')
                [CompletionResult]::new('debug', 'debug', [CompletionResultType]::ParameterValue, 'Parse & resolve, but don''t lower into RQ')
                [CompletionResult]::new('resolve', 'resolve', [CompletionResultType]::ParameterValue, 'Parse, resolve & lower into RQ')
                [CompletionResult]::new('sql:preprocess', 'sql:preprocess', [CompletionResultType]::ParameterValue, 'Parse, resolve, lower into RQ & preprocess SRQ')
                [CompletionResult]::new('sql:anchor', 'sql:anchor', [CompletionResultType]::ParameterValue, 'Parse, resolve, lower into RQ & preprocess & anchor SRQ')
                [CompletionResult]::new('compile', 'compile', [CompletionResultType]::ParameterValue, 'Parse, resolve, lower into RQ & compile to SQL')
                [CompletionResult]::new('watch', 'watch', [CompletionResultType]::ParameterValue, 'Watch a directory and compile .prql files to .sql files')
                [CompletionResult]::new('list-targets', 'list-targets', [CompletionResultType]::ParameterValue, 'Show available compile target names')
                [CompletionResult]::new('shell-completion', 'shell-completion', [CompletionResultType]::ParameterValue, 'Print a shell completion for supported shells')
                [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
                break
            }
            'prqlc;help;parse' {
                break
            }
            'prqlc;help;fmt' {
                break
            }
            'prqlc;help;annotate' {
                break
            }
            'prqlc;help;debug' {
                break
            }
            'prqlc;help;resolve' {
                break
            }
            'prqlc;help;sql:preprocess' {
                break
            }
            'prqlc;help;sql:anchor' {
                break
            }
            'prqlc;help;compile' {
                break
            }
            'prqlc;help;watch' {
                break
            }
            'prqlc;help;list-targets' {
                break
            }
            'prqlc;help;shell-completion' {
                break
            }
            'prqlc;help;help' {
                break
            }
        })

        $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
            Sort-Object -Property ListItemText
    }

    ----- stderr -----
    "###);
}
