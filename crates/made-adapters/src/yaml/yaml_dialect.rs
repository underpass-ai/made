//! [`YamlDialect`] — the one YAML reading and writing every MADE crate uses.
//!
//! The parser is `serde-saphyr`. Its defaults are YAML 1.1 flavoured and
//! stricter than the parser MADE used before (`serde_yaml` 0.9), so a
//! document that was accepted, and the values it produced, would have
//! changed under an author's feet. The options below keep the old
//! reading for every value a ceremony can hold:
//!
//! - only `true`/`false` are booleans; `yes`, `no`, `on`, `off` stay text;
//! - `007` and `1_000` stay text, as YAML 1.2 says;
//! - `~`, `null` or nothing written into a text field is that text;
//! - a repeated key is handed to the receiving type: a struct refuses
//!   it, a map keeps the last value;
//! - `<<` is an ordinary key, not a merge;
//! - `.inf` and `.nan` reach the receiving type, which for a JSON value
//!   means `null`.
//!
//! What is kept from the new parser on purpose: its budget against
//! alias bombs and oversized documents. Errors carry the line and
//! column, without the source snippet, so a caller that shows the
//! message shows one line.

use serde::de::DeserializeOwned;
use serde::Serialize;

use super::yaml_dialect_error::YamlDialectError;

#[derive(Debug, Default, Clone, Copy)]
pub struct YamlDialect;

impl YamlDialect {
    /// Read one YAML document into `T`.
    pub fn parse<T: DeserializeOwned>(raw: &str) -> Result<T, YamlDialectError> {
        serde_saphyr::from_str_with_options(raw, Self::options())
            .map_err(|error| YamlDialectError::new(error.to_string()))
    }

    /// Write `value` as one YAML document.
    ///
    /// Text that would read back as another type (`"yes"`, `"007"`,
    /// `"null"`) is quoted, so the document reads back to `value` both
    /// here and in a `serde_yaml` reader. Long lines stay on one line, as
    /// they did before: folding a prompt at eighty columns would make a
    /// person reviewing a diff read every line of it again. Multi-line
    /// text is a literal block. Quoted text uses double quotes where the
    /// old writer used single ones; the value is the same.
    pub fn render<T: Serialize>(value: &T) -> Result<String, YamlDialectError> {
        let mut options = serde_saphyr::SerializerOptions::default();
        options.folded_wrap_chars = usize::MAX;
        serde_saphyr::to_string_with_options(value, options)
            .map_err(|error| YamlDialectError::new(error.to_string()))
    }

    fn options() -> serde_saphyr::Options {
        let mut options = serde_saphyr::Options::default();
        options.strict_booleans = true;
        options.leading_zero_numbers_as_strings = true;
        options.underscored_numbers_as_strings = true;
        options.duplicate_keys = serde_saphyr::DuplicateKeyPolicy::PassThrough;
        options.merge_keys = serde_saphyr::MergeKeyPolicy::AsOrdinary;
        options.non_finite_float_policy = serde_saphyr::NonFiniteFloatPolicy::PassThrough;
        options.null_as_string = true;
        options.with_snippet = false;
        options
    }
}

#[cfg(test)]
mod tests {
    use super::YamlDialect;
    use serde_json::{json, Value};

    #[test]
    fn untyped_values_read_as_they_did_before() {
        let value: Value = YamlDialect::parse(
            "a: yes\nb: off\nc: 007\nd: 1_000\ne: 0x1F\nf: ~\ng: .inf\nh: True\n",
        )
        .unwrap();

        assert_eq!(
            value,
            json!({"a": "yes", "b": "off", "c": "007", "d": "1_000", "e": 31,
                   "f": null, "g": null, "h": true})
        );
    }

    #[test]
    fn a_null_spelling_in_a_text_field_is_that_text() {
        #[derive(Debug, serde::Deserialize)]
        struct Document {
            a: String,
            b: String,
            c: Option<String>,
        }

        let document: Document = YamlDialect::parse("a: ~\nb:\nc: null\n").unwrap();

        assert_eq!((document.a.as_str(), document.b.as_str()), ("~", ""));
        assert_eq!(document.c, None);
    }

    #[test]
    fn a_repeated_key_keeps_the_last_value_in_a_map() {
        let value: Value = YamlDialect::parse("a: 1\na: 2\n").unwrap();

        assert_eq!(value, json!({"a": 2}));
    }

    #[test]
    fn a_repeated_field_is_refused_by_a_struct() {
        #[derive(Debug, serde::Deserialize)]
        #[allow(dead_code)]
        struct Document {
            a: u32,
        }

        assert!(YamlDialect::parse::<Document>("a: 1\na: 2\n").is_err());
    }

    #[test]
    fn a_merge_key_is_an_ordinary_key() {
        let value: Value = YamlDialect::parse("base: &b {x: 1}\nm:\n  <<: *b\n  y: 2\n").unwrap();

        assert_eq!(value["m"], json!({"<<": {"x": 1}, "y": 2}));
    }

    #[test]
    fn an_alias_bomb_is_refused() {
        use std::fmt::Write as _;

        let mut bomb = String::from("a0: &a0 [x,x,x,x,x,x,x,x,x,x]\n");
        for level in 1..10 {
            let previous = level - 1;
            let items = std::iter::repeat_n(format!("*a{previous}"), 10)
                .collect::<Vec<_>>()
                .join(",");
            writeln!(bomb, "a{level}: &a{level} [{items}]").unwrap();
        }

        assert!(YamlDialect::parse::<Value>(&bomb).is_err());
    }

    #[test]
    fn errors_are_one_line() {
        let error = YamlDialect::parse::<Value>("version: [").unwrap_err();

        assert!(!error.to_string().contains('\n'), "{error}");
    }

    #[test]
    fn rendered_text_reads_back_as_text() {
        let value = json!({"a": "yes", "b": "007", "c": "null", "d": "true",
                           "e": "1.0", "f": "<<", "g": "multi\nline\n", "h": 1.5,
                           "i": "word ".repeat(40)});

        let rendered = YamlDialect::render(&value).unwrap();

        assert_eq!(YamlDialect::parse::<Value>(&rendered).unwrap(), value);
        assert!(rendered.contains("g: |"), "{rendered}");
        assert!(!rendered.contains("i: >"), "{rendered}");
    }
}
