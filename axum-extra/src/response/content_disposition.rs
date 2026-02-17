use std::fmt::{self, Write};

/// A wrapper type that escapes backslashes and double quotes when formatted,
/// for safe inclusion in Content-Disposition header quoted-strings.
///
/// This prevents Content-Disposition header parameter injection
/// (similar to CVE-2023-29401).
pub(crate) struct EscapedFilename<'a>(pub &'a str);

impl fmt::Display for EscapedFilename<'_> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_special_characters() {
        assert_eq!(EscapedFilename("report.pdf").to_string(), "report.pdf");
    }

    #[test]
    fn escapes_double_quotes() {
        assert_eq!(
            EscapedFilename("evil\"; filename*=UTF-8''pwned.txt; x=\"").to_string(),
            "evil\\\"; filename*=UTF-8''pwned.txt; x=\\\"",
        );
    }

    #[test]
    fn escapes_backslashes() {
        assert_eq!(
            EscapedFilename("file\\name.txt").to_string(),
            "file\\\\name.txt",
        );
    }
}
