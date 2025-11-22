=begin pod

=head1 NAME

prql.rakumod - Grammar for PRQL.

=head1 SYNOPSIS

    use PRQL;

    # Parse a simple PRQL query
    say PRQL.parse('from employees');

    # Parse a PRQL file
    say PRQL.parsefile('employees.prql');

=head1 DESCRIPTION

PRQL grammar for Raku.

PRQL is a modern language for transforming data — a simple, powerful, pipelined SQL replacement.

=head1 SEE ALSO

L<https://prql-lang.org/> and L<https://github.com/PRQL/prql/>.

=end pod

grammar PRQL {
    token TOP {
        <statement>*
    }
    
    rule statement {
        | <doc-block>
        | <comment>
        | <query-definition>
        | <module>
        | <annotation>
        | <variable-declaration>
        | <pipeline-statement> <comment>?
    }
    
    rule query-definition {
        'prql' <named-arg>+
    }
    
    rule module {
        'module' <identifier> '{' <statement>* '}'
    }
    
    rule pipeline-statement {
        <pipeline>
    }
    
    rule pipeline {
        | <call-expression>+ % '|'
        | <expression> '|' <identifier>
    }
    
    rule tuple-expression {
        '{'
        (
        | <declaration-tuple>
        | <call-expression>
        | <expression>
        | <case-branch>
        )* %% ','
        '}'
    }
    
    rule annotation {
        '@{'
        <declaration>? % ','
        '}'
    }
    
    rule call-expression {
        <identifier>
        (
        | <named-arg>
        | <declaration>
        | <test>
        )+
    }
    
    rule named-arg {
        <identifier> ':' <expression>
    }
    
    rule declaration {
        <identifier> '=' <expression>
    }
    
    rule declaration-tuple {
        <identifier> '=' <expression>
    }

    rule case-branch {
        <expression> '=>' <expression>
    }
    
    rule case-expression {
        'case' <tuple-expression>
    }
    
    rule nested-pipeline {
        '(' <pipeline> ')'
    }
    
    # The name "test" here means equality testing. It is the name used in the Python grammar.
    rule test {
        <test-inner>
    }
    
    rule test-inner {
        | <binary-test>
        | '!' <test-inner>
        | <expression>
    }
    
    rule binary-test {
        | <expression> <logic-op> <expression>
        | <expression> <compare-op> <expression>
        | <expression> <arith-op> <expression>
    }
    
    token expression {
        | 'this'
        | 'that'
        | 'null'
        | <binary-expression>
        | <unary-expression>
        | <array-expression>
        | <tuple-expression>
        | <nested-pipeline>
        | <case-expression>
        | <date-time>
        | <parameter>
        | <parenthesized-expression>
        | <range-expression>
        | <identifier>
        | <boolean>
        | <time-unit>
        | <number>
        | <string>
        | <f-string>
        | <r-string>
        | <s-string>
    }
    
    token boolean {
        | 'true'
        | 'false'
    }
    
    token variable-name {
        <.ident>
    }
    
    token identifier {
        <.ident>
        ['.' [<.ident> | '*']]*
    }
    
    rule array-expression {
        '['
        (<test> | '*' <expression>)+ %% ','
        ']'
    }
    
    rule binary-expression {
        <identifier> <arith-op> <expression>
    }
    
    rule parenthesized-expression {
        '(' <expression> ')'
    }
    
    rule unary-expression {
        | ('+' | '-') <expression>
        | '==' <identifier>
    }
    
    token number {
        | <float>
        | <integer>
    }
    
    rule variable-declaration {
        'let' <variable-name> '=' (<nested-pipeline> | <lambda>)
    }
    
    rule lambda {
        <lambda-param>* '->' <expression>
    }

    rule type-definition {
        '<' <type-name> ('|' <type-name>)* '>'
    }

    rule type-name {
        <identifier> <type-definition>?
    }

    rule lambda-param {
        <identifier> <type-definition>? (':' <expression>)?
    }
    
    token integer {
        | <.digit> [<.digit> | '_']* ['e' ['+' | '-']? <integer>]?
        | '0x' [<.xdigit> | '_']+
        | '0b' <[01_]>+
        | '0o' <[0..7_]>+
    }

    token float {
        \d [\d | '_']* '.' \d [\d | '_']* ['e' <integer>]?
    }
    
    token date {
        \d+ '-' \d+ '-' \d+
    }

    token time {
        \d+ ':' \d+
        [':' \d+ ['.' \d+]?]?
    }
    
    token date-time {
        '@'
        (
        | <date> 'T' <time> ['Z' | ['-' | '+'] \d+ ':' \d+]?
        | <date>
        | <time>
        )
    }
    
    token time-unit {
        $<number>=\d+
        <dimension>
    }
    
    token escape {
        '\\'
        (
        | 'x' <xdigit> <xdigit>
        | 'u' '{' <xdigit>+ '}'
        | <[bfnrt]>
        )
    }
    
    token parameter {
        '$'
        \w+
    }
    
    token doc-block {
        '#!' .+? $$
    }
    
    token comment {
        '#' .+? $$
    }
    
    token range-expression {
        (<.digit>+)
        '..'
        (<.digit>+)
    }
    
    token dimension {
        [
        | microseconds
        | milliseconds
        | seconds
        | minutes
        | hours
        | days
        | weeks
        | months
        | years
        ] <!ww>
    }

    token arith-op {
        '+' | '-' | '*' | '/' | '%' | '//' | '**'
    }
    
    token compare-op {
        '==' | '!=' | '~=' | '>=' | '<=' | '>' | '<' | 'in'
    }
    
    token logic-op {
        '&&' | '||' | '??'
    }
    
    token f-string {
        | 'f"' (<-[\"]> | <escape>)* '"'
        | 'f\'' (<-[\']> | <escape>)* '\''
    }
    
    token r-string {
        | 'r"' <-[\"]>* '"'
        | 'r\'' <-[\']>* '\''
    }
    
    token s-string {
        | 's"' (<-[\"]> | <escape>)* '"'
        | 's\'' (<-[\']> | <escape>)* '\''
    }
    
    token string {
        | '"""' (<-[\"]> | <escape>)* '"""'
        | '"' (<-[\"]> | <escape>)* '"'
        | '\'' (<-[\']> | <escape>)* '\''
        | '\'\'\'' (<-[\']> | <escape>)* '\'\'\''
    }
}
