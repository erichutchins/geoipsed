use std::collections::HashMap;
use std::fmt;

/// A pre-compiled template for fast rendering.
///
/// Templates use `{field_name}` syntax for field references.
/// Use `{{` to produce a literal `{` in output.
///
/// The template is parsed once at compile time into a sequence of literal
/// and field segments. Rendering is a single left-to-right pass that
/// concatenates segments — no double-substitution is possible.
#[derive(Clone, Debug)]
pub struct Template {
    parts: Vec<TemplatePart>,
    /// Pre-computed estimate of output size for allocation.
    estimated_size: usize,
}

#[derive(Clone, Debug)]
enum TemplatePart {
    Literal(String),
    Field(String),
}

/// Error returned when a template string is malformed.
#[derive(Debug)]
pub struct TemplateError {
    pub reason: String,
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid template: {}", self.reason)
    }
}

impl std::error::Error for TemplateError {}

impl Template {
    /// Compile a template string into a pre-parsed representation.
    ///
    /// Field references are `{field_name}`. Use `{{` for a literal `{`.
    /// An unclosed `{` (no matching `}`) is treated as a literal.
    ///
    /// # Errors
    ///
    /// Returns a `TemplateError` if the template contains an empty field name (`{}`).
    pub fn compile(template: &str) -> Result<Template, TemplateError> {
        let mut parts = Vec::new();
        let mut literal = String::new();
        let mut estimated_size = 0;
        let bytes = template.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            if bytes[i] == b'{' {
                if i + 1 < len && bytes[i + 1] == b'{' {
                    // Escaped brace: {{ → {
                    literal.push('{');
                    i += 2;
                    continue;
                }
                // Look for closing brace
                if let Some(close) = template[i + 1..].find('}') {
                    let field_name = &template[i + 1..i + 1 + close];
                    if field_name.is_empty() {
                        return Err(TemplateError {
                            reason: "empty field name at position".to_string(),
                        });
                    }
                    // Flush accumulated literal
                    if !literal.is_empty() {
                        estimated_size += literal.len();
                        parts.push(TemplatePart::Literal(std::mem::take(&mut literal)));
                    }
                    // Estimate ~16 bytes per field value
                    estimated_size += 16;
                    parts.push(TemplatePart::Field(field_name.to_string()));
                    i += 1 + close + 1; // skip past }
                } else {
                    // No closing brace — treat as literal
                    literal.push('{');
                    i += 1;
                }
            } else if bytes[i] == b'}' && i + 1 < len && bytes[i + 1] == b'}' {
                // Escaped closing brace: }} → }
                literal.push('}');
                i += 2;
            } else {
                literal.push(bytes[i] as char);
                i += 1;
            }
        }

        // Flush remaining literal
        if !literal.is_empty() {
            estimated_size += literal.len();
            parts.push(TemplatePart::Literal(literal));
        }

        Ok(Template {
            parts,
            estimated_size,
        })
    }

    /// Render the template using a closure to look up field values.
    ///
    /// The closure receives a field name and returns the value to substitute.
    /// This is a single-pass operation — values are never re-scanned for
    /// field references, so double-substitution cannot occur.
    #[inline]
    pub fn render<'a>(&self, mut lookup: impl FnMut(&str) -> &'a str) -> String {
        let mut output = String::with_capacity(self.estimated_size);
        for part in &self.parts {
            match part {
                TemplatePart::Literal(s) => output.push_str(s),
                TemplatePart::Field(name) => output.push_str(lookup(name)),
            }
        }
        output
    }

    /// Renders the template and writes it to the writer.
    ///
    /// The closure receives the writer and a field name, and should write
    /// the corresponding value to the writer.
    ///
    /// # Errors
    ///
    /// Returns `std::io::Result` if writing to the provided writer fails.
    #[inline]
    pub fn write<W, L>(&self, wtr: &mut W, mut lookup: L) -> std::io::Result<()>
    where
        W: std::io::Write + ?Sized,
        L: FnMut(&mut W, &str) -> std::io::Result<()>,
    {
        for part in &self.parts {
            match part {
                TemplatePart::Literal(s) => wtr.write_all(s.as_bytes())?,
                TemplatePart::Field(f) => {
                    lookup(wtr, f)?;
                }
            }
        }
        Ok(())
    }

    /// Render the template with a `HashMap` of field values.
    ///
    /// Unknown fields are replaced with an empty string.
    #[inline]
    #[must_use]
    pub fn render_with_map(&self, values: &HashMap<String, String>) -> String {
        self.render(move |name| values.get(name).map_or("", |s| s.as_str()))
    }

    /// Get the list of field names referenced in this template.
    #[must_use]
    pub fn fields(&self) -> Vec<&str> {
        self.parts
            .iter()
            .filter_map(|part| match part {
                TemplatePart::Field(name) => Some(name.as_str()),
                TemplatePart::Literal(_) => None,
            })
            .collect()
    }
}

impl fmt::Display for Template {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for part in &self.parts {
            match part {
                TemplatePart::Literal(s) => write!(f, "{s}")?,
                TemplatePart::Field(name) => write!(f, "{{{name}}}")?,
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_template() {
        let t = Template::compile("<{ip}|{country}>").unwrap();
        let mut values = HashMap::new();
        values.insert("ip".to_string(), "1.2.3.4".to_string());
        values.insert("country".to_string(), "US".to_string());
        assert_eq!(t.render_with_map(&values), "<1.2.3.4|US>");
    }

    #[test]
    fn multiple_fields() {
        let t = Template::compile("{a}-{b}-{c}").unwrap();
        let result = t.render(|name| match name {
            "a" => "1",
            "b" => "2",
            "c" => "3",
            _ => "",
        });
        assert_eq!(result, "1-2-3");
    }

    #[test]
    fn empty_template() {
        let t = Template::compile("").unwrap();
        assert_eq!(t.render(|_| "unused"), "");
    }

    #[test]
    fn all_literal() {
        let t = Template::compile("no fields here").unwrap();
        assert_eq!(t.render(|_| "unused"), "no fields here");
    }

    #[test]
    fn escaped_braces() {
        let t = Template::compile("{{literal}} and {field}").unwrap();
        assert_eq!(t.render(|_| "val"), "{literal} and val");
    }

    #[test]
    fn escaped_closing_brace() {
        let t = Template::compile("value is }}done").unwrap();
        assert_eq!(t.render(|_| ""), "value is }done");
    }

    #[test]
    fn no_double_substitution() {
        // Critical test: a field value containing {other_field} must NOT be expanded
        let t = Template::compile("{a} and {b}").unwrap();
        let result = t.render(|name| match name {
            "a" => "{b}",
            "b" => "real_b",
            _ => "",
        });
        assert_eq!(result, "{b} and real_b");
    }

    #[test]
    fn unknown_fields_empty() {
        let t = Template::compile("{known} {unknown}").unwrap();
        let mut values = HashMap::new();
        values.insert("known".to_string(), "yes".to_string());
        assert_eq!(t.render_with_map(&values), "yes ");
    }

    #[test]
    fn fields_method() {
        let t = Template::compile("{ip}|{asnnum}_{asnorg}|{country_iso}").unwrap();
        assert_eq!(t.fields(), vec!["ip", "asnnum", "asnorg", "country_iso"]);
    }

    #[test]
    fn unclosed_brace_is_literal() {
        let t = Template::compile("value is {unclosed").unwrap();
        assert_eq!(t.render(|_| ""), "value is {unclosed");
    }

    #[test]
    fn display_roundtrip() {
        let template_str = "<{ip}|AS{asnnum}_{asnorg}|{country_iso}|{city}>";
        let t = Template::compile(template_str).unwrap();
        assert_eq!(t.to_string(), template_str);
    }

    #[test]
    fn empty_field_name_is_error() {
        assert!(Template::compile("{}").is_err());
    }

    #[test]
    fn geoipsed_default_template() {
        let t = Template::compile("<{ip}|AS{asnnum}_{asnorg}|{country_iso}|{city}>").unwrap();
        let result = t.render(|name| match name {
            "ip" => "93.184.216.34",
            "asnnum" => "15133",
            "asnorg" => "EDGECAST",
            "country_iso" => "US",
            "city" => "Los_Angeles",
            _ => "",
        });
        assert_eq!(result, "<93.184.216.34|AS15133_EDGECAST|US|Los_Angeles>");
    }
}
