use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxConfig {
    pub root_marker: String,
    pub aliases: Vec<String>,
}

impl Default for SyntaxConfig {
    fn default() -> Self {
        Self {
            root_marker: "rune".to_owned(),
            aliases: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct SyntaxError {
    message: String,
}

#[derive(Debug)]
pub struct LowerError {
    message: String,
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl SyntaxConfig {
    pub fn load(path: &Path) -> Result<Self, SyntaxError> {
        let source = fs::read_to_string(path).map_err(|error| SyntaxError {
            message: format!("cannot read `{}`: {error}", path.display()),
        })?;
        Self::parse(&source).map_err(|error| SyntaxError {
            message: format!("{}: {error}", path.display()),
        })
    }

    pub fn discover(source_path: &Path) -> Option<PathBuf> {
        let absolute = source_path
            .canonicalize()
            .unwrap_or_else(|_| source_path.to_path_buf());
        let mut directory = absolute.parent();
        while let Some(current) = directory {
            let candidate = current.join("Runes.toml");
            if candidate.is_file() {
                return Some(candidate);
            }
            directory = current.parent();
        }
        None
    }

    pub fn parse(source: &str) -> Result<Self, SyntaxError> {
        let mut config = Self::default();
        let mut in_syntax = false;

        for (index, raw_line) in source.lines().enumerate() {
            let line_number = index + 1;
            let line = strip_comment(raw_line).trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') {
                in_syntax = line == "[syntax]";
                continue;
            }
            if !in_syntax {
                continue;
            }

            let (key, value) = line.split_once('=').ok_or_else(|| SyntaxError {
                message: format!("line {line_number}: expected `key = value`"),
            })?;
            match key.trim() {
                "root_marker" => config.root_marker = parse_string(value.trim(), line_number)?,
                "aliases" => config.aliases = parse_string_array(value.trim(), line_number)?,
                other => {
                    return Err(SyntaxError {
                        message: format!("line {line_number}: unknown syntax option `{other}`"),
                    });
                }
            }
        }

        config.validate()?;
        Ok(config)
    }

    pub fn is_identifier_marker(&self, identifier: &str) -> bool {
        self.markers()
            .any(|marker| is_identifier(marker) && marker == identifier)
    }

    pub fn is_symbol_marker(&self, symbol: char) -> bool {
        self.markers().any(|marker| {
            let mut chars = marker.chars();
            chars.next() == Some(symbol) && chars.next().is_none() && !is_identifier(marker)
        })
    }

    pub fn display_marker(&self) -> &str {
        &self.root_marker
    }

    fn markers(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.root_marker.as_str()).chain(self.aliases.iter().map(String::as_str))
    }

    fn validate(&self) -> Result<(), SyntaxError> {
        for marker in self.markers() {
            if marker.is_empty() {
                return Err(SyntaxError {
                    message: "rune marker cannot be empty".to_owned(),
                });
            }
            if !is_identifier(marker) && marker.chars().count() != 1 {
                return Err(SyntaxError {
                    message: format!(
                        "marker `{marker}` must be an identifier or one Unicode character"
                    ),
                });
            }
        }
        Ok(())
    }
}

/// Lowers the Runes contextual root declaration into ordinary Rust.
///
/// Everything except a declaration shaped like `rune name = expression;` is
/// copied byte-for-byte. Rust still parses and type-checks the resulting code.
pub fn lower_rust_source(source: &str, syntax: &SyntaxConfig) -> Result<String, LowerError> {
    let markers = std::iter::once(syntax.root_marker.as_str())
        .chain(syntax.aliases.iter().map(String::as_str))
        .collect::<Vec<_>>();
    let mut output = String::with_capacity(source.len());
    let mut copied_until = 0;
    let mut cursor = 0;

    while cursor < source.len() {
        if let Some(end) = skipped_lexeme_end(source, cursor) {
            cursor = end;
            continue;
        }

        let mut lowered = None;
        for marker in &markers {
            if !source[cursor..].starts_with(marker) || !marker_boundary(source, cursor, marker) {
                continue;
            }
            if let Some(binding) = parse_rune_binding(source, cursor, marker)? {
                lowered = Some(binding);
                break;
            }
        }

        if let Some(binding) = lowered {
            let line = source[..cursor]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1;
            let column = source[..cursor]
                .rsplit_once('\n')
                .map_or(cursor + 1, |(_, tail)| tail.chars().count() + 1);
            output.push_str(&source[copied_until..cursor]);
            output.push_str("let ");
            output.push_str(binding.name);
            output.push_str(" = ::runes_runtime::mark_root({ ");
            output.push_str(binding.expression.trim());
            output.push_str(" }, ::runes_runtime::SourceSite::new(\"");
            output.push_str(binding.name);
            output.push_str("\", ::core::file!(), ");
            output.push_str(&line.to_string());
            output.push_str(", ");
            output.push_str(&column.to_string());
            output.push_str("));");
            cursor = binding.end;
            copied_until = cursor;
        } else {
            cursor += source[cursor..].chars().next().unwrap().len_utf8();
        }
    }

    output.push_str(&source[copied_until..]);
    Ok(output)
}

struct RuneBinding<'a> {
    name: &'a str,
    expression: &'a str,
    end: usize,
}

fn parse_rune_binding<'a>(
    source: &'a str,
    start: usize,
    marker: &str,
) -> Result<Option<RuneBinding<'a>>, LowerError> {
    let mut cursor = start + marker.len();
    let whitespace_start = cursor;
    cursor = skip_whitespace(source, cursor);
    if cursor == whitespace_start {
        return Ok(None);
    }

    let Some(name_end) = identifier_end(source, cursor) else {
        return Ok(None);
    };
    let name = &source[cursor..name_end];
    cursor = skip_whitespace(source, name_end);
    if source.as_bytes().get(cursor) != Some(&b'=')
        || source.as_bytes().get(cursor + 1) == Some(&b'=')
    {
        return Ok(None);
    }
    let expression_start = cursor + 1;
    let expression_end = expression_end(source, expression_start).ok_or_else(|| LowerError {
        message: format!("unterminated rune declaration for `{name}`"),
    })?;
    Ok(Some(RuneBinding {
        name,
        expression: &source[expression_start..expression_end],
        end: expression_end + 1,
    }))
}

fn expression_end(source: &str, mut cursor: usize) -> Option<usize> {
    let mut parentheses = 0_u32;
    let mut brackets = 0_u32;
    let mut braces = 0_u32;
    while cursor < source.len() {
        if let Some(end) = skipped_lexeme_end(source, cursor) {
            cursor = end;
            continue;
        }
        let ch = source[cursor..].chars().next()?;
        match ch {
            '(' => parentheses += 1,
            ')' => parentheses = parentheses.saturating_sub(1),
            '[' => brackets += 1,
            ']' => brackets = brackets.saturating_sub(1),
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            ';' if parentheses == 0 && brackets == 0 && braces == 0 => return Some(cursor),
            _ => {}
        }
        cursor += ch.len_utf8();
    }
    None
}

fn skipped_lexeme_end(source: &str, start: usize) -> Option<usize> {
    let rest = &source[start..];
    if rest.starts_with("//") {
        return Some(
            rest.find('\n')
                .map_or(source.len(), |offset| start + offset + 1),
        );
    }
    if rest.starts_with("/*") {
        let mut depth = 1_u32;
        let mut cursor = start + 2;
        while cursor < source.len() {
            if source[cursor..].starts_with("/*") {
                depth += 1;
                cursor += 2;
            } else if source[cursor..].starts_with("*/") {
                depth -= 1;
                cursor += 2;
                if depth == 0 {
                    return Some(cursor);
                }
            } else {
                cursor += source[cursor..].chars().next()?.len_utf8();
            }
        }
        return Some(source.len());
    }
    if let Some(end) = raw_string_end(source, start) {
        return Some(end);
    }
    if rest.starts_with('"') {
        return Some(quoted_end(source, start, '"'));
    }
    if rest.starts_with('\'') && looks_like_char_literal(source, start) {
        return Some(quoted_end(source, start, '\''));
    }
    None
}

fn raw_string_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    if matches!(bytes.get(cursor), Some(b'b' | b'c')) {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'r') {
        return None;
    }
    cursor += 1;
    let hashes_start = cursor;
    while bytes.get(cursor) == Some(&b'#') {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'"') {
        return None;
    }
    let hashes = cursor - hashes_start;
    cursor += 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'"'
            && bytes.get(cursor + 1..cursor + 1 + hashes)
                == Some(&bytes[hashes_start..hashes_start + hashes])
        {
            return Some(cursor + 1 + hashes);
        }
        cursor += 1;
    }
    Some(source.len())
}

fn quoted_end(source: &str, start: usize, quote: char) -> usize {
    let mut escaped = false;
    for (offset, ch) in source[start + quote.len_utf8()..].char_indices() {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            return start + quote.len_utf8() + offset + ch.len_utf8();
        }
    }
    source.len()
}

fn looks_like_char_literal(source: &str, start: usize) -> bool {
    let rest = &source[start + 1..];
    let mut chars = rest.chars();
    match chars.next() {
        Some('\\') => chars.next().is_some() && chars.next() == Some('\''),
        Some(_) => chars.next() == Some('\''),
        None => false,
    }
}

fn marker_boundary(source: &str, start: usize, marker: &str) -> bool {
    if is_identifier(marker) {
        let before_ok = source[..start]
            .chars()
            .next_back()
            .is_none_or(|ch| ch != '_' && !ch.is_alphanumeric());
        let after_ok = source[start + marker.len()..]
            .chars()
            .next()
            .is_none_or(|ch| ch != '_' && !ch.is_alphanumeric());
        before_ok && after_ok
    } else {
        true
    }
}

fn skip_whitespace(source: &str, mut cursor: usize) -> usize {
    while let Some(ch) = source[cursor..].chars().next() {
        if !ch.is_whitespace() {
            break;
        }
        cursor += ch.len_utf8();
        if cursor == source.len() {
            break;
        }
    }
    cursor
}

fn identifier_end(source: &str, start: usize) -> Option<usize> {
    let mut chars = source[start..].char_indices();
    let (_, first) = chars.next()?;
    if first != '_' && !first.is_alphabetic() {
        return None;
    }
    let mut end = start + first.len_utf8();
    for (offset, ch) in chars {
        if ch != '_' && !ch.is_alphanumeric() {
            break;
        }
        end = start + offset + ch.len_utf8();
    }
    Some(end)
}

fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '#' if !in_string => return &line[..index],
            _ => {}
        }
    }
    line
}

fn parse_string(value: &str, line: usize) -> Result<String, SyntaxError> {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        Ok(value[1..value.len() - 1].to_owned())
    } else {
        Err(SyntaxError {
            message: format!("line {line}: expected a quoted string"),
        })
    }
}

fn parse_string_array(value: &str, line: usize) -> Result<Vec<String>, SyntaxError> {
    let Some(inner) = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    else {
        return Err(SyntaxError {
            message: format!("line {line}: expected an array of quoted strings"),
        });
    };
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    inner
        .split(',')
        .map(|item| parse_string(item.trim(), line))
        .collect()
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keyword_and_symbol_aliases() {
        let config = SyntaxConfig::parse(
            r#"
            [syntax]
            root_marker = "rune"
            aliases = ["◇", "trace"]
            "#,
        )
        .unwrap();

        assert!(config.is_identifier_marker("rune"));
        assert!(config.is_identifier_marker("trace"));
        assert!(config.is_symbol_marker('◇'));
    }

    #[test]
    fn lowers_keyword_declarations_but_not_strings_or_function_calls() {
        let source = r###"
            let text = "rune ignored = 1;";
            rune user = User {
                name: r#"Ada;Lovelace"#.to_owned(),
            };
            rune();
        "###;
        let lowered = lower_rust_source(source, &SyntaxConfig::default()).unwrap();

        assert!(lowered.contains("let user = ::runes_runtime::mark_root"));
        assert!(lowered.contains("rune ignored = 1;"));
        assert!(lowered.contains("rune();"));
    }

    #[test]
    fn lowers_a_configured_symbol_alias() {
        let syntax = SyntaxConfig {
            root_marker: "rune".to_owned(),
            aliases: vec!["◇".to_owned()],
        };
        let lowered = lower_rust_source("◇ user = make_user();", &syntax).unwrap();
        assert!(lowered.starts_with("let user = ::runes_runtime::mark_root"));
    }
}
