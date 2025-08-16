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
        last = token.span.end;
        output.push_str(&" ".repeat(diff));
        output.push_str(&highlight_token_kind(&token.kind));
    }

    output
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
        TokenKind::Range {
            bind_left: _,
            bind_right: _,
        } => output.push_str(".."),
        TokenKind::Interpolation(_, _) => output.push_str(&format!("{}", token.yellow())),
        TokenKind::Control(char) => output.push(*char),
        TokenKind::ArrowThin
        | TokenKind::ArrowFat
        | TokenKind::Eq
        | TokenKind::Ne
        | TokenKind::Gte
        | TokenKind::Lte
        | TokenKind::RegexSearch => output.push_str(&format!("{token}")),
        TokenKind::And | TokenKind::Or => {
            output.push_str(&format!("{token}").purple().to_string())
        }
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
