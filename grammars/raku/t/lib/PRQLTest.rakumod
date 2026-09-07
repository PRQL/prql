unit module PRQLTest;

use Test;
use prql;

#| Render a parse tree as `rule(child,child)`, naming only the grammar's named
#| captures. A capture with no named children of its own carries the text it
#| matched in `«…»`, so tests that differ only in a leaf — the five compare
#| operators, the four string flavours — assert different shapes rather than
#| one shared string. The `( … )` groups the grammar uses for grouping and
#| quantification are transparent: their named children appear in place of the
#| group itself.
sub shape(Match:D $match --> Str) is export {
    children($match).join(',')
}

sub children(Match:D $m --> List) {
    my @out;
    for $m.caps -> $cap {
        my $name = $cap.key.Str;
        if $name ~~ /^ \d+ $/ {
            @out.append: children($cap.value);
        } else {
            my @kids = children($cap.value);
            @out.push: @kids
                ?? $name ~ '(' ~ @kids.join(',') ~ ')'
                !! $name ~ '«' ~ $cap.value.Str ~ '»';
        }
    }
    @out.List
}

#| Break a shape onto one line per capture, indented by nesting depth, for the
#| failure diagnostic. `is` prints a shape as a single line, which for the
#| whole-query test is over 2000 characters and unreadable next to its
#| counterpart. Text inside `«…»` is copied through untouched so a `(`, `)` or
#| `,` in a matched string doesn't shift the indentation.
sub indented(Str:D $shape --> Str) {
    my $out = '';
    my $depth = 0;
    my $in-text = False;
    for $shape.comb -> $c {
        if $in-text {
            $out ~= $c;
            $in-text = False if $c eq '»';
        } elsif $c eq '«' {
            $out ~= $c;
            $in-text = True;
        } elsif $c eq '(' {
            $depth++;
            $out ~= "\n" ~ '  ' x $depth;
        } elsif $c eq ')' {
            $depth--;
        } elsif $c eq ',' {
            $out ~= "\n" ~ '  ' x $depth;
        } else {
            $out ~= $c;
        }
    }
    $out
}

#| Parse `$source` and check the tree against `$expected`, so a test covers the
#| structure the grammar builds and not only that the parse succeeded. The TAP
#| description defaults to the first line of `$source`, which is enough for the
#| single-line tests; pass `:desc` for the heredoc ones, whose first line is
#| both truncated and — for pairs like `derive {` in `t/misc.rakutest` — shared
#| with another test.
sub parses-to(Str:D $source, Str:D $expected, Str :$desc --> Bool) is export {
    my $description = $desc // $source.trim.lines.head;
    my $match = PRQL.parse($source);
    without $match {
        flunk $description;
        diag "did not parse: $source";
        return False;
    }
    my $got = shape($match);
    # `ok` rather than `is`: the diagnostic below replaces the two single-line
    # `expected`/`got` dumps `is` would print.
    my $passed = ok $got eq $expected, $description;
    unless $passed {
        diag "got:\n" ~ indented($got);
        diag "expected:\n" ~ indented($expected);
    }
    $passed
}
