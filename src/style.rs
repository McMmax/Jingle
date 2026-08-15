use std::collections::HashMap;
use std::sync::RwLock;

/// a named color specification
#[derive(Debug, Clone, PartialEq)]
pub enum ColorSpec {
    Named(&'static str),
    Hex(u32),
    Legacy(char),
}

impl ColorSpec {
    fn to_legacy_code(&self) -> Option<char> {
        match self {
            Self::Legacy(c) => Some(*c),
            _ => None,
        }
    }
}

/// a style preset — builder pattern
#[derive(Debug, Clone, Default)]
pub struct Style {
    pub color: Option<ColorSpec>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub gradient: Option<(String, String)>,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn color(mut self, spec: &str) -> Self {
        self.color = Some(parse_color_spec(spec));
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    pub fn gradient(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.gradient = Some((from.into(), to.into()));
        self
    }

    /// converts to a legacy & code prefix — only works for single-char legacy colors
    pub fn to_legacy_prefix(&self) -> String {
        let mut out = String::new();
        if let Some(c) = self.color.as_ref().and_then(|c| c.to_legacy_code()) {
            out.push('&');
            out.push(c);
        }
        if self.bold        { out.push_str("&l"); }
        if self.italic      { out.push_str("&o"); }
        if self.underline   { out.push_str("&n"); }
        if self.strikethrough { out.push_str("&m"); }
        out
    }

    /// converts to MiniMessage opening tag(s) for downstream component parsing
    pub fn to_mini_message(&self) -> String {
        let mut tags = String::new();
        if let Some(grad) = &self.gradient {
            return format!("<gradient:{}:{}>", grad.0, grad.1);
        }
        if let Some(color) = &self.color {
            match color {
                ColorSpec::Hex(h)    => tags.push_str(&format!("<color:#{h:06X}>")),
                ColorSpec::Named(n)  => tags.push_str(&format!("<{n}>")),
                ColorSpec::Legacy(c) => {
                    if let Some(name) = legacy_to_name(*c) {
                        tags.push_str(&format!("<{name}>"));
                    }
                }
            }
        }
        if self.bold          { tags.push_str("<bold>"); }
        if self.italic        { tags.push_str("<italic>"); }
        if self.underline     { tags.push_str("<underlined>"); }
        if self.strikethrough { tags.push_str("<strikethrough>"); }
        tags
    }
}

fn parse_color_spec(s: &str) -> ColorSpec {
    if s.starts_with('#') && s.len() == 7 {
        if let Ok(v) = u32::from_str_radix(&s[1..], 16) {
            return ColorSpec::Hex(v);
        }
    }
    if s.len() == 1 {
        let c = s.chars().next().unwrap();
        if is_legacy_code(c) {
            return ColorSpec::Legacy(c);
        }
    }
    // SAFE(max): leak is intentional — named colors are &'static and come from short strings
    ColorSpec::Named(Box::leak(s.to_owned().into_boxed_str()))
}

fn is_legacy_code(c: char) -> bool {
    matches!(c, '0'..='9' | 'a'..='f' | 'r' | 'k'..='o')
}

fn legacy_to_name(c: char) -> Option<&'static str> {
    Some(match c {
        '0' => "black",       '1' => "dark_blue",    '2' => "dark_green",
        '3' => "dark_aqua",   '4' => "dark_red",      '5' => "dark_purple",
        '6' => "gold",        '7' => "gray",          '8' => "dark_gray",
        '9' => "blue",        'a' => "green",         'b' => "aqua",
        'c' => "red",         'd' => "light_purple",  'e' => "yellow",
        'f' => "white",
        _ => return None,
    })
}

/// thread-safe registry of named style presets
#[derive(Default)]
pub struct StyleRegistry {
    inner: RwLock<HashMap<String, Style>>,
}

impl StyleRegistry {
    pub fn new() -> Self {
        Self { inner: RwLock::new(HashMap::with_capacity(16)) }
    }

    pub fn register(&self, name: impl Into<String>, style: Style) {
        self.inner.write().unwrap().insert(name.into(), style);
    }

    pub fn get(&self, name: &str) -> Option<Style> {
        self.inner.read().unwrap().get(name).cloned()
    }

    pub fn names(&self) -> Vec<String> {
        self.inner.read().unwrap().keys().cloned().collect()
    }
}

impl std::fmt::Debug for StyleRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StyleRegistry({} styles)", self.inner.read().unwrap().len())
    }
}

/// strips all & legacy color codes from a string
pub fn strip_legacy_codes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            if let Some(&next) = chars.peek() {
                if is_legacy_code(next) {
                    chars.next();
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

/// strips MiniMessage `<tag>` and `</tag>` markers — does not parse nested structure
pub fn strip_mini_message_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => { in_tag = true; }
            '>' => { in_tag = false; }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// strips both & codes and MiniMessage tags — for plain text output (console, logs)
pub fn strip_all_formatting(s: &str) -> String {
    strip_mini_message_tags(&strip_legacy_codes(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_legacy_prefix() {
        let s = Style::new().color("c").bold();
        assert_eq!(s.to_legacy_prefix(), "&c&l");
    }

    #[test]
    fn style_mini_message_hex() {
        let s = Style::new().color("#FF5555").bold();
        assert!(s.to_mini_message().contains("<color:#FF5555>"));
        assert!(s.to_mini_message().contains("<bold>"));
    }

    #[test]
    fn style_mini_message_gradient() {
        let s = Style::new().gradient("#FF0000", "#0000FF");
        assert_eq!(s.to_mini_message(), "<gradient:#FF0000:#0000FF>");
    }

    #[test]
    fn strip_removes_codes() {
        assert_eq!(strip_legacy_codes("&aHello &r&lWorld"), "Hello World");
    }

    #[test]
    fn strip_preserves_ampersand_without_code() {
        assert_eq!(strip_legacy_codes("5 & 10"), "5 & 10");
    }

    #[test]
    fn strip_mini_message_removes_tags() {
        assert_eq!(strip_mini_message_tags("<red>hello</red>"), "hello");
        assert_eq!(strip_mini_message_tags("<color:#FF5555><bold>text</bold></color:#FF5555>"), "text");
    }

    #[test]
    fn strip_all_formatting_removes_both() {
        assert_eq!(strip_all_formatting("&a<red>hello</red>"), "hello");
    }

    #[test]
    fn registry_register_and_get() {
        let reg = StyleRegistry::new();
        reg.register("error", Style::new().color("#FF5555").bold());
        let s = reg.get("error").unwrap();
        assert!(s.bold);
        assert_eq!(s.color, Some(ColorSpec::Hex(0xFF5555)));
    }

    #[test]
    fn registry_get_missing_returns_none() {
        let reg = StyleRegistry::new();
        assert!(reg.get("nope").is_none());
    }

    #[test]
    fn hex_color_parses() {
        let s = Style::new().color("#FF5500");
        assert_eq!(s.color, Some(ColorSpec::Hex(0xFF5500)));
    }
}
