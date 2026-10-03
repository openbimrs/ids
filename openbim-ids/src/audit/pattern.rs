//! XML Schema regular expressions, translated for the `regex` crate.
//!
//! XSD patterns are implicitly anchored, treat `^` and `$` as ordinary
//! characters, and have constructs `regex` lacks (`\i`, `\c`, character-class
//! subtraction, `\p{Is…}` blocks). Those are refused rather than
//! approximated, so a pattern either matches exactly as XSD would or is
//! reported as unverified.

use regex::Regex;

/// Compiles an XSD pattern, or says why it cannot be checked here.
pub(super) fn compile(pattern: &str) -> Result<Regex, String> {
    let mut out = String::with_capacity(pattern.len() + 8);
    out.push_str("^(?:");
    let mut chars = pattern.chars().peekable();
    let mut class_depth = 0usize;
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let Some(escaped) = chars.next() else {
                    return Err("a trailing backslash".into());
                };
                match escaped {
                    'i' | 'I' | 'c' | 'C' => {
                        return Err(format!("the XML name escape \\{escaped}"));
                    }
                    'p' | 'P' if chars.peek() == Some(&'{') => {
                        let name: String = chars.clone().take_while(|&c| c != '}').collect();
                        if name.trim_start_matches('{').starts_with("Is") {
                            return Err("a Unicode block escape (\\p{Is…})".into());
                        }
                        out.push('\\');
                        out.push(escaped);
                    }
                    _ => {
                        out.push('\\');
                        out.push(escaped);
                    }
                }
            }
            '[' => {
                class_depth += 1;
                out.push('[');
            }
            ']' if class_depth > 0 => {
                class_depth -= 1;
                out.push(']');
            }
            '-' if class_depth > 0 && chars.peek() == Some(&'[') => {
                return Err("character-class subtraction".into());
            }
            '^' if class_depth == 0 => out.push_str("\\^"),
            '$' => out.push_str("\\$"),
            // XSD `.` excludes both line ends.
            '.' if class_depth == 0 => out.push_str("[^\\n\\r]"),
            c => out.push(c),
        }
    }
    out.push_str(")$");
    Regex::new(&out).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::compile;

    #[test]
    fn patterns_are_anchored_and_literal_where_xsd_says_so() {
        let re = compile("IFC.*TYPE").unwrap();
        assert!(re.is_match("IFCWALLTYPE"));
        assert!(!re.is_match("XIFCWALLTYPE"));
        assert!(!re.is_match("IFCWALLTYPES"));
        let re = compile("a^b$").unwrap();
        assert!(re.is_match("a^b$"));
        let re = compile("[^a]b").unwrap();
        assert!(re.is_match("xb") && !re.is_match("ab"));
        let re = compile("NumberOfRiser(s)?").unwrap();
        assert!(re.is_match("NumberOfRisers") && re.is_match("NumberOfRiser"));
        assert!(!compile("a.b").unwrap().is_match("a\rb"));
    }

    #[test]
    fn constructs_without_an_equivalent_are_refused() {
        for pattern in ["\\i\\c*", "[a-z-[aeiou]]", "\\p{IsBasicLatin}", "x\\"] {
            assert!(compile(pattern).is_err(), "{pattern}");
        }
        assert!(compile("\\p{Lu}+").is_ok());
    }
}
