unit module PRQLTest;

use Test;
use prql;

#| Render a parse tree as `rule(child,child)`, naming only the grammar's named
#| captures. The `( … )` groups the grammar uses for grouping and
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
            @out.push: @kids ?? $name ~ '(' ~ @kids.join(',') ~ ')' !! $name;
        }
    }
    @out.List
}

#| Parse `$source` and check the tree against `$expected`, so a test covers the
#| structure the grammar builds and not only that the parse succeeded.
sub parses-to(Str:D $source, Str:D $expected, Str :$desc --> Bool) is export {
    my $description = $desc // $source.trim.lines.head;
    my $match = PRQL.parse($source);
    without $match {
        flunk $description;
        diag "did not parse: $source";
        return False;
    }
    is shape($match), $expected, $description
}
