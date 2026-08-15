mod parse;
mod eval;

pub use parse::TemplateError;

use crate::placeholder::RenderContext;
use crate::style::StyleRegistry;
use eval::{eval_nodes, eval_nodes_full};
use parse::parse_nodes;

/// a parsed, validated template tree — pre-compiled, not re-parsed on render
#[derive(Debug, Clone)]
pub struct Template {
    pub(crate) nodes: Vec<TemplateNode>,
    pub(crate) raw: String,
}

/// a single node in the parsed template tree
#[derive(Debug, Clone)]
pub enum TemplateNode {
    Literal(String),
    Placeholder(PlaceholderExpr),
    Conditional { key: String, negated: bool, body: Vec<TemplateNode> },
    StyledBlock { style: String, body: Vec<TemplateNode> },
    GradientBlock { from: String, to: String, body: Vec<TemplateNode> },
    HexBlock { hex: String, body: Vec<TemplateNode> },
    KeyRef(String),
}

/// a placeholder expression — key with zero or more modifiers
#[derive(Debug, Clone)]
pub struct PlaceholderExpr {
    pub key: String,
    pub modifiers: Vec<Modifier>,
}

/// modifier applied after placeholder resolution
#[derive(Debug, Clone)]
pub enum Modifier {
    Plural { singular: String, plural: String },
    Default(String),
    Join(String),
    Fallback(String),
    Style(String),
}

impl Template {
    /// parses template source text — allocates once, renders many times
    pub fn parse(src: &str) -> Result<Self, TemplateError> {
        let nodes = parse_nodes(src)?;
        Ok(Self { nodes, raw: src.to_owned() })
    }

    /// renders the template against a context — no catalog lookup, no style registry
    pub fn render(&self, ctx: &RenderContext) -> String {
        eval_nodes(&self.nodes, ctx)
    }

    /// renders with a catalog lookup for {key:path} refs, no style registry
    pub fn render_with_catalog<F>(&self, ctx: &RenderContext, lookup: F) -> String
    where
        F: Fn(&str) -> Option<String>,
    {
        eval_nodes_full(&self.nodes, ctx, &lookup, None)
    }

    /// renders with both a catalog lookup and a style registry — full evaluation
    pub fn render_full<F>(&self, ctx: &RenderContext, lookup: F, style_reg: Option<&StyleRegistry>) -> String
    where
        F: Fn(&str) -> Option<String>,
    {
        eval_nodes_full(&self.nodes, ctx, &lookup, style_reg)
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }
}
