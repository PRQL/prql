use color_eyre::owo_colors::OwoColorize;
use prqlc::{
    lr::{TokenKind, Tokens},
    pr::Literal,
};

/// Highlight PRQL code printed to the terminal.
pub fn highlight(tokens: &Tokens) -> String {
    let mut output = String::new();
    let mut last = 0;

    for token in &tokens.0 {
        let diff = token.span.start - last;
        last = token.span.end;
        output.push_str(&" ".repeat(diff));

        match &token.kind {
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
            TokenKind::Interpolation(_, _) => output.push_str(&format!("{}", token.kind.yellow())),
            TokenKind::Control(char) => output.push(*char),
            TokenKind::ArrowThin
            | TokenKind::ArrowFat
            | TokenKind::Eq
            | TokenKind::Ne
            | TokenKind::Gte
            | TokenKind::Lte
            | TokenKind::RegexSearch => output.push_str(&format!("{}", token.kind)),
            TokenKind::And | TokenKind::Or => {
                output.push_str(&format!("{}", token.kind).purple().to_string())
            }
            TokenKind::Coalesce | TokenKind::DivInt | TokenKind::Pow | TokenKind::Annotate => {
                output.push_str(&format!("{}", token.kind))
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
            TokenKind::LineWrap(_) => todo!(),
            TokenKind::Start => {}
        }
    }

    output
}

fn is_transform(ident: &str) -> bool {
    // TODO: Could we instead source these from the standard library?
    // We could also use the semantic understanding from later compiler stages?
    match ident {
        "from" => true,
        "derive" | "select" | "filter" | "sort" | "join" | "take" | "group" | "aggregate"
        | "window" | "loop" => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use insta_cmd::assert_cmd_snapshot;
    use insta_cmd::get_cargo_bin;

    #[test]
    fn highlight() {
        assert_cmd_snapshot!(prqlc_command().args(["experimental", "highlight"]).pass_stdin("from tracks"), @r###"
        success: true
        exit_code: 0
        ----- stdout -----
        from tracks
        ----- stderr -----
        "###);
    }

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
