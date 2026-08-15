use super::{Modifier, PlaceholderExpr, TemplateNode};

#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum TemplateError {
    #[error("unclosed tag at position {pos}: {tag}")]
    UnclosedTag { pos: usize, tag: String },
    #[error("unexpected closing tag at position {pos}: {tag}")]
    UnexpectedClose { pos: usize, tag: String },
    #[error("malformed tag at position {pos}: {src}")]
    MalformedTag { pos: usize, src: String },
    #[error("invalid hex color: {src}")]
    InvalidHex { src: String },
}

/// parses template source into a flat-to-nested node list
pub fn parse_nodes(src: &str) -> Result<Vec<TemplateNode>, TemplateError> {
    let mut parser = Parser::new(src);
    parser.parse_until(None)
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    fn remaining(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn advance(&mut self, n: usize) {
        self.pos += n;
    }

    /// parses until a specific closing tag (or end of input)
    pub fn parse_until(&mut self, close_tag: Option<&str>) -> Result<Vec<TemplateNode>, TemplateError> {
        let mut nodes: Vec<TemplateNode> = Vec::new();
        let mut literal_start = self.pos;

        while self.pos < self.src.len() {
            let rem = self.remaining();

            if let Some(open) = rem.find('{') {
                // flush any leading literal
                let lit = &rem[..open];
                if !lit.is_empty() {
                    nodes.push(TemplateNode::Literal(lit.to_owned()));
                }
                self.advance(open);

                let tag_start = self.pos;
                let rem2 = self.remaining();
                let close = rem2.find('}').ok_or_else(|| TemplateError::UnclosedTag {
                    pos: tag_start,
                    tag: rem2.chars().take(20).collect(),
                })?;

                let tag_inner = &rem2[1..close]; // between { and }
                self.advance(close + 1);
                literal_start = self.pos;

                // check for closing tags first
                if let Some(name) = tag_inner.strip_prefix('/') {
                    if let Some(expected) = close_tag {
                        if name == expected || name.is_empty() {
                            return Ok(nodes);
                        }
                    }
                    return Err(TemplateError::UnexpectedClose {
                        pos: tag_start,
                        tag: tag_inner.to_owned(),
                    });
                }

                let node = self.parse_tag(tag_inner, tag_start)?;
                // block-level nodes consume their body recursively
                match node {
                    OpenTag::IfBlock { key, negated } => {
                        let body = self.parse_until(Some("if"))?;
                        nodes.push(TemplateNode::Conditional { key, negated, body });
                    }
                    OpenTag::StyleBlock { style } => {
                        let body = self.parse_until(Some("style"))?;
                        nodes.push(TemplateNode::StyledBlock { style, body });
                    }
                    OpenTag::GradientBlock { from, to } => {
                        let body = self.parse_until(Some("gradient"))?;
                        nodes.push(TemplateNode::GradientBlock { from, to, body });
                    }
                    OpenTag::HexBlock { hex } => {
                        let body = self.parse_until(Some(""))?; // closed by {/}
                        nodes.push(TemplateNode::HexBlock { hex, body });
                    }
                    OpenTag::Leaf(n) => nodes.push(n),
                }
            } else {
                // no more tags — rest is a literal
                let lit = rem;
                if !lit.is_empty() {
                    nodes.push(TemplateNode::Literal(lit.to_owned()));
                }
                self.pos = self.src.len();
            }
        }

        if let Some(expected) = close_tag {
            if !expected.is_empty() {
                return Err(TemplateError::UnclosedTag {
                    pos: literal_start,
                    tag: format!("/{expected}"),
                });
            }
        }

        Ok(nodes)
    }

    fn parse_tag(&self, inner: &str, pos: usize) -> Result<OpenTag, TemplateError> {
        // {if:key} / {if:!key}
        if let Some(rest) = inner.strip_prefix("if:") {
            let negated = rest.starts_with('!');
            let key = if negated { rest[1..].to_owned() } else { rest.to_owned() };
            return Ok(OpenTag::IfBlock { key, negated });
        }

        // {/style}, {/if} etc — handled by caller, shouldn't reach here
        if inner.starts_with('/') {
            return Err(TemplateError::UnexpectedClose { pos, tag: inner.to_owned() });
        }

        // {style:name}
        if let Some(name) = inner.strip_prefix("style:") {
            return Ok(OpenTag::StyleBlock { style: name.to_owned() });
        }

        // {gradient:#RRGGBB:#RRGGBB}
        if let Some(rest) = inner.strip_prefix("gradient:") {
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            if parts.len() != 2 {
                return Err(TemplateError::MalformedTag { pos, src: inner.to_owned() });
            }
            return Ok(OpenTag::GradientBlock {
                from: parts[0].to_owned(),
                to: parts[1].to_owned(),
            });
        }

        // {#RRGGBB} hex inline color block
        if inner.starts_with('#') && inner.len() == 7 {
            validate_hex(&inner[1..], pos)?;
            return Ok(OpenTag::HexBlock { hex: inner.to_owned() });
        }

        // {key:msg.path} — nested key reference
        if let Some(path) = inner.strip_prefix("key:") {
            return Ok(OpenTag::Leaf(TemplateNode::KeyRef(path.to_owned())));
        }

        // plain placeholder with optional modifiers: {key|mod1:arg|mod2:a:b}
        let node = parse_placeholder(inner, pos)?;
        Ok(OpenTag::Leaf(TemplateNode::Placeholder(node)))
    }
}

enum OpenTag {
    IfBlock { key: String, negated: bool },
    StyleBlock { style: String },
    GradientBlock { from: String, to: String },
    HexBlock { hex: String },
    Leaf(TemplateNode),
}

fn parse_placeholder(src: &str, pos: usize) -> Result<PlaceholderExpr, TemplateError> {
    let mut parts = src.splitn(2, '|');
    let key = parts.next().unwrap_or("").trim().to_owned();
    if key.is_empty() {
        return Err(TemplateError::MalformedTag { pos, src: src.to_owned() });
    }

    let mut modifiers = Vec::new();
    if let Some(mod_str) = parts.next() {
        for seg in mod_str.split('|') {
            if let Some(modifier) = parse_modifier(seg) {
                modifiers.push(modifier);
            }
        }
    }

    Ok(PlaceholderExpr { key, modifiers })
}

fn parse_modifier(seg: &str) -> Option<Modifier> {
    // plural::s  → Plural { singular: "", plural: "s" }
    // plural:friend:friends → Plural { singular: "friend", plural: "friends" }
    if let Some(rest) = seg.strip_prefix("plural:") {
        let parts: Vec<&str> = rest.splitn(2, ':').collect();
        return Some(match parts.as_slice() {
            [suffix] => Modifier::Plural { singular: String::new(), plural: suffix.to_string() },
            [singular, plural] => Modifier::Plural {
                singular: singular.to_string(),
                plural: plural.to_string(),
            },
            _ => return None,
        });
    }
    if let Some(val) = seg.strip_prefix("default:") {
        return Some(Modifier::Default(val.to_owned()));
    }
    if let Some(sep) = seg.strip_prefix("join:") {
        return Some(Modifier::Join(sep.to_owned()));
    }
    if let Some(val) = seg.strip_prefix("fallback:") {
        return Some(Modifier::Fallback(val.to_owned()));
    }
    if let Some(name) = seg.strip_prefix("style:") {
        return Some(Modifier::Style(name.to_owned()));
    }
    None
}

fn validate_hex(s: &str, _pos: usize) -> Result<(), TemplateError> {
    if s.len() != 6 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(TemplateError::InvalidHex { src: format!("#{s}") });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_literal() {
        let nodes = parse_nodes("hello world").unwrap();
        assert!(matches!(&nodes[0], TemplateNode::Literal(s) if s == "hello world"));
    }

    #[test]
    fn parses_placeholder() {
        let nodes = parse_nodes("{username}").unwrap();
        assert!(matches!(&nodes[0], TemplateNode::Placeholder(p) if p.key == "username"));
    }

    #[test]
    fn parses_default_modifier() {
        let nodes = parse_nodes("{username|default:Unknown}").unwrap();
        if let TemplateNode::Placeholder(p) = &nodes[0] {
            assert!(matches!(&p.modifiers[0], Modifier::Default(v) if v == "Unknown"));
        } else {
            panic!("expected placeholder");
        }
    }

    #[test]
    fn parses_if_block() {
        let nodes = parse_nodes("{if:in_party}yes{/if}").unwrap();
        assert!(matches!(&nodes[0], TemplateNode::Conditional { key, negated: false, .. } if key == "in_party"));
    }

    #[test]
    fn parses_negated_if() {
        let nodes = parse_nodes("{if:!in_queue}no queue{/if}").unwrap();
        assert!(matches!(&nodes[0], TemplateNode::Conditional { negated: true, .. }));
    }

    #[test]
    fn parses_key_ref() {
        let nodes = parse_nodes("{key:staff.prefix}").unwrap();
        assert!(matches!(&nodes[0], TemplateNode::KeyRef(k) if k == "staff.prefix"));
    }

    #[test]
    fn unclosed_brace_errors() {
        assert!(parse_nodes("{unclosed").is_err());
    }

    #[test]
    fn unclosed_if_errors() {
        assert!(parse_nodes("{if:flag}body").is_err());
    }

    #[test]
    fn mixed_literal_and_placeholder() {
        let nodes = parse_nodes("Hello {name}!").unwrap();
        assert_eq!(nodes.len(), 3);
    }
}
