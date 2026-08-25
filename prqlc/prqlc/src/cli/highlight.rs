use color_eyre::owo_colors::OwoColorize;
use prqlc::{
    lr::{TokenKind, Tokens},
    pr::Literal,
};

/// Highlight PRQL code printed to the terminal.
pub(crate) fn highlight(tokens: &Tokens) -> String {
    let mut output = String::new();
    let mut last = 0;

    for token in &tokens.0 {
        let diff = token.span.start - last;
        output.push_str(&" ".repeat(diff));
        // A range's span covers the whitespace either side of the `..`, and
        // that whitespace decides whether the range binds — so it's the one
        // kind whose rendering needs the span. (`LineWrap` spans are wider
        // than their text too, but the whitespace they swallow is inert.)
        match &token.kind {
            TokenKind::Range {
                bind_left,
                bind_right,
            } => output.push_str(&highlight_range(*bind_left, *bind_right, token.span.len())),
            kind => output.push_str(&highlight_token_kind(kind)),
        }
        last = token.span.end;
    }

    output
}

/// Render a range token, including the whitespace either side of the `..`.
///
/// The lexer folds that whitespace into the range token's own span, and it's
/// what decides whether the range binds: `take 1..5` compiles, while
/// `take 1 .. 5` is an error. Writing back a bare `..` would print a different
/// program than the one being highlighted.
///
/// `width` is the span's width, which the padding fills so that everything
/// later on the line keeps its column.
fn highlight_range(bind_left: bool, bind_right: bool, width: usize) -> String {
    let min_left = usize::from(!bind_left);
    let min_right = usize::from(!bind_right);
    // Whatever the span holds beyond the minimum is whitespace on an unbound
    // side. When both sides are unbound the split isn't recoverable from the
    // token, so it's split evenly — which round-trips symmetric spacing like
    // `1  ..  5` exactly, and only re-splits genuinely asymmetric spacing.
    let surplus = width.saturating_sub("..".len() + min_left + min_right);
    let (left, right) = if bind_left {
        (0, min_right + surplus)
    } else if bind_right {
        (min_left + surplus, 0)
    } else {
        (min_left + surplus.div_ceil(2), min_right + surplus / 2)
    };
    format!("{}..{}", " ".repeat(left), " ".repeat(right))
}

fn highlight_token_kind(token: &TokenKind) -> String {
    // LineWrap is recursive with TokenKind, so we needed to split this function
    // out from the one above (otherwise would have it as a single func)
    let mut output = String::new();
    match &token {
        TokenKind::NewLine => output.push('\n'),
        TokenKind::Ident(ident) => {
            if is_transform(ident) {
                output.push_str(&ident.green().to_string())
            } else {
                output.push_str(&ident.to_string())
            }
        }
        TokenKind::Keyword(keyword) => output.push_str(&keyword.blue().to_string()),
        TokenKind::Literal(literal) => output.push_str(&match literal {
            Literal::Null => literal.green().bold().to_string(),
            Literal::Integer(_) => literal.green().to_string(),
            Literal::Float(_) => literal.green().to_string(),
            Literal::Boolean(_) => literal.green().bold().to_string(),
            Literal::String(_) => literal.yellow().to_string(),
            _ => literal.to_string(),
        }),
        TokenKind::Param(param) => output.push_str(&param.purple().to_string()),
        // Unreachable: `highlight` intercepts `Range` before dispatching here,
        // and a `LineWrap` only ever carries comments. Kept for exhaustiveness;
        // width `0` renders the minimum padding.
        TokenKind::Range {
            bind_left,
            bind_right,
        } => output.push_str(&highlight_range(*bind_left, *bind_right, 0)),
        TokenKind::Interpolation(_, _) => output.push_str(&format!("{}", token.yellow())),
        TokenKind::Control(char) => output.push(*char),
        TokenKind::ArrowThin
        | TokenKind::ArrowFat
        | TokenKind::Eq
        | TokenKind::Ne
        | TokenKind::Gte
        | TokenKind::Lte
        | TokenKind::RegexSearch => output.push_str(&format!("{token}")),
        TokenKind::And | TokenKind::Or => output.push_str(&format!("{token}").purple().to_string()),
        TokenKind::Coalesce | TokenKind::DivInt | TokenKind::Pow | TokenKind::Annotate => {
            output.push_str(&format!("{token}"))
        }
        TokenKind::Comment(comment) => output.push_str(
            &format!("#{comment}")
                .truecolor(95, 135, 135)
                .italic()
                .to_string(),
        ),
        TokenKind::DocComment(comment) => output.push_str(
            &format!("#!{comment}")
                .truecolor(95, 135, 135)
                .italic()
                .to_string(),
        ),
        TokenKind::LineWrap(inner_tokens) => {
            output.push_str("\n\\");
            for t in inner_tokens {
                output.push_str(&highlight_token_kind(t));
            }
        }
        TokenKind::Start => {}
    }
    output
}

fn is_transform(ident: &str) -> bool {
    // TODO: Could we instead source these from the standard library?
    // We could also use the semantic understanding from later compiler stages?
    matches!(
        ident,
        "from"
            | "derive"
            | "select"
            | "filter"
            | "sort"
            | "join"
            | "take"
            | "group"
            | "aggregate"
            | "window"
            | "loop"
    )
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use insta_cmd::assert_cmd_snapshot;
    use insta_cmd::get_cargo_bin;

    #[test]
    fn highlight() {
        // (Colors don't show because they're disabled; we could have a test
        // that forces them to show?)
        assert_cmd_snapshot!(prqlc_command().args(["experimental", "highlight"]).pass_stdin(r#"
        from tracks
        filter artist == "Bob Marley"                 # Each line transforms the previous result
        aggregate {                                   # `aggregate` reduces each column to a value
          plays    = sum plays,
          longest  = max length,
          shortest = min length,                      # Trailing commas are allowed
        }

        "#), @r#"
        success: true
        exit_code: 0
        ----- stdout -----

                from tracks
                filter artist == "Bob Marley"                 # Each line transforms the previous result
                aggregate {                                   # `aggregate` reduces each column to a value
                  plays    = sum plays,
                  longest  = max length,
                  shortest = min length,                      # Trailing commas are allowed
                }


        ----- stderr -----
        "#);
    }

    /// The whitespace around `..` is part of the range token's span, and it
    /// decides whether the range binds — `take 1..5` compiles, `take 1 .. 5`
    /// doesn't — so highlighting has to preserve it rather than print a bare
    /// `..`.
    #[test]
    fn highlight_range_whitespace() {
        assert_cmd_snapshot!(prqlc_command().args(["experimental", "highlight"]).pass_stdin(r#"from x
take 1..5
take 1 .. 5
take 1   ..5
take 1..   5
take 1  ..  5
"#), @r"
        success: true
        exit_code: 0
        ----- stdout -----
        from x
        take 1..5
        take 1 .. 5
        take 1   ..5
        take 1..   5
        take 1  ..  5

        ----- stderr -----
        ");
    }

    // TODO: import from existing location, need to adjust visibility
    fn prqlc_command() -> Command {
        let mut cmd = Command::new(get_cargo_bin("prqlc"));
        normalize_prqlc(&mut cmd);
        cmd
    }

    fn normalize_prqlc(cmd: &mut Command) -> &mut Command {
        cmd
            // We set `CLICOLOR_FORCE` in CI to force color output, but we don't want `prqlc` to
            // output color for our snapshot tests. And it seems to override the
            // `--color=never` flag.
            .env_remove("CLICOLOR_FORCE")
            .env("NO_COLOR", "1")
            .args(["--color=never"])
            // We don't want the tests to be affected by the user's `RUST_BACKTRACE` setting.
            .env_remove("RUST_BACKTRACE")
            .env_remove("RUST_LOG")
    }
}
