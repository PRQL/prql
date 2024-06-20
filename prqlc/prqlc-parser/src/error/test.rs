#[cfg(test)]
mod tests {
    use insta::{assert_debug_snapshot, assert_snapshot};

    use crate::error::{Error, Errors, Reason, WithErrorInfo};

    // Helper function to create a simple Error object
    fn create_simple_error() -> Error {
        Error::new_simple("A simple error message")
            .push_hint("take a hint")
            .with_code("E001")
    }

    #[test]
    fn display() {
        assert_snapshot!(create_simple_error(),
            @r###"Error { kind: Error, span: None, reason: Simple("A simple error message"), hints: ["take a hint"], code: Some("E001") }"###
        );

        let errors = Errors(vec![create_simple_error()]);
        assert_snapshot!(errors,
            @r###"Errors([Error { kind: Error, span: None, reason: Simple("A simple error message"), hints: ["take a hint"], code: Some("E001") }])"###
        );
        assert_debug_snapshot!(errors, @r###"
        Errors(
            [
                Error {
                    kind: Error,
                    span: None,
                    reason: Simple(
                        "A simple error message",
                    ),
                    hints: [
                        "take a hint",
                    ],
                    code: Some(
                        "E001",
                    ),
                },
            ],
        )
        "###)
    }

    #[test]
    fn test_simple_error() {
        let err = create_simple_error();
        assert_debug_snapshot!(err, @r###"
        Error {
            kind: Error,
            span: None,
            reason: Simple(
                "A simple error message",
            ),
            hints: [
                "take a hint",
            ],
            code: Some(
                "E001",
            ),
        }
        "###);
    }

    #[test]
    fn test_complex_error() {
        assert_debug_snapshot!(
        Error::new(Reason::Expected {
            who: Some("Test".to_string()),
            expected: "expected_value".to_string(),
            found: "found_value".to_string(),
        })
        .with_code("E002"), @r###"
        Error {
            kind: Error,
            span: None,
            reason: Expected {
                who: Some(
                    "Test",
                ),
                expected: "expected_value",
                found: "found_value",
            },
            hints: [],
            code: Some(
                "E002",
            ),
        }
        "###);
    }

    #[test]
    fn test_simple_error_with_result() {
        let result: Result<(), Error> = Err(Error::new_simple("A simple error message"))
            .with_hints(vec!["Take a hint"])
            .push_hint("Take another hint")
            .with_code("E001");
        assert_debug_snapshot!(result, @r###"
        Err(
            Error {
                kind: Error,
                span: None,
                reason: Simple(
                    "A simple error message",
                ),
                hints: [
                    "Take a hint",
                    "Take another hint",
                ],
                code: Some(
                    "E001",
                ),
            },
        )
        "###);
    }
}
