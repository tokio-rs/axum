use std::fmt::{self, Write};

/// A wrapper type that escapes backslashes and double quotes when formatted,
/// for safe inclusion in Content-Disposition header quoted-strings.
///
/// This prevents Content-Disposition header parameter injection
/// (similar to CVE-2023-29401).
pub(crate) struct EscapedQuotedString<'a>(pub &'a str);

impl fmt::Display for EscapedQuotedString<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.0.chars() {
            if c == '\\' || c == '"' {
                f.write_char('\\')?;
            }
            f.write_char(c)?;
        }
        Ok(())
    }
}

#[cfg(any(feature = "multipart", test))]
pub(crate) fn contains_newlines(value: &str) -> bool {
    value.contains(['\r', '\n'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_special_characters() {
        assert_eq!(EscapedQuotedString("report.pdf").to_string(), "report.pdf");
    }

    #[test]
    fn escapes_double_quotes() {
        assert_eq!(
            EscapedQuotedString("evil\"; filename*=UTF-8''pwned.txt; x=\"").to_string(),
            "evil\\\"; filename*=UTF-8''pwned.txt; x=\\\"",
        );
    }

    #[test]
    fn escapes_backslashes() {
        assert_eq!(
            EscapedQuotedString("file\\name.txt").to_string(),
            "file\\\\name.txt",
        );
    }

    #[test]
    fn detects_newlines() {
        assert!(contains_newlines("line\r\nbreak"));
        assert!(contains_newlines("line\nbreak"));
        assert!(!contains_newlines("report.pdf"));
    }
}
