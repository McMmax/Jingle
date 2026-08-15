use super::{Modifier, PlaceholderExpr, TemplateNode};
use crate::placeholder::RenderContext;
use crate::style::StyleRegistry;

/// evaluate nodes without a catalog lookup or style registry
pub fn eval_nodes(nodes: &[TemplateNode], ctx: &RenderContext) -> String {
    eval_nodes_full(nodes, ctx, &|_| None, None)
}

/// evaluate nodes with a catalog lookup and a style registry for {style:name} blocks
pub fn eval_nodes_full<F>(
    nodes: &[TemplateNode],
    ctx: &RenderContext,
    lookup: &F,
    style_reg: Option<&StyleRegistry>,
) -> String
where
    F: Fn(&str) -> Option<String>,
{
    // PERF(max): pre-size with rough estimate to avoid reallocs on typical messages
    let mut out = String::with_capacity(nodes.len() * 16);
    for node in nodes {
        match node {
            TemplateNode::Literal(s) => out.push_str(s),
            TemplateNode::Placeholder(expr) => {
                out.push_str(&eval_placeholder(expr, ctx));
            }
            TemplateNode::Conditional { key, negated, body } => {
                let present = ctx.contains(key);
                let show = if *negated { !present } else { present };
                if show {
                    out.push_str(&eval_nodes_full(body, ctx, lookup, style_reg));
                }
            }
            TemplateNode::StyledBlock { style, body } => {
                let inner = eval_nodes_full(body, ctx, lookup, style_reg);
                if let Some(reg) = style_reg {
                    if let Some(s) = reg.get(style) {
                        // POLISH(max): <reset> resets all formatting after the block
                        out.push_str(&s.to_mini_message());
                        out.push_str(&inner);
                        out.push_str("<reset>");
                        continue;
                    }
                }
                // style not registered or no registry — emit body as-is
                out.push_str(&inner);
            }
            TemplateNode::GradientBlock { from, to, body } => {
                let inner = eval_nodes_full(body, ctx, lookup, style_reg);
                out.push_str(&format!("<gradient:{from}:{to}>{inner}</gradient>"));
            }
            TemplateNode::HexBlock { hex, body } => {
                let inner = eval_nodes_full(body, ctx, lookup, style_reg);
                // MiniMessage hex format for downstream component parsing
                out.push_str(&format!("<{hex}>{inner}</{hex}>"));
            }
            TemplateNode::KeyRef(path) => {
                if let Some(val) = lookup(path) {
                    out.push_str(&val);
                }
                // silently omit unknown refs — may resolve later via store
            }
        }
    }
    out
}

fn eval_placeholder(expr: &PlaceholderExpr, ctx: &RenderContext) -> String {
    let raw = ctx.resolve(&expr.key);
    let mut value = raw.clone();

    for modifier in &expr.modifiers {
        match modifier {
            Modifier::Default(fallback) => {
                if value.is_none() {
                    value = Some(fallback.clone());
                }
            }
            Modifier::Fallback(fallback) => {
                if value.as_deref().map(|s| s.is_empty()).unwrap_or(true) {
                    value = Some(fallback.clone());
                }
            }
            Modifier::Join(sep) => {
                if let Some(list) = ctx.get(&expr.key).and_then(|v| v.as_list()) {
                    value = Some(list.join(sep));
                }
            }
            Modifier::Plural { singular, plural } => {
                if let Some(n) = parse_count(value.as_deref().unwrap_or("0")) {
                    // replace value with the suffix — caller places {count} before this expression
                    let suffix = if n == 1 { singular } else { plural };
                    value = Some(suffix.clone());
                }
            }
            Modifier::Style(_) => {
                // style modifier is consumed by the renderer, not the text layer
            }
        }
    }

    value.unwrap_or_default()
}

fn parse_count(s: &str) -> Option<i64> {
    s.trim().parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placeholder::RenderContext;
    use crate::style::{Style, StyleRegistry};
    use crate::template::parse::parse_nodes;

    fn render(src: &str, ctx: &RenderContext) -> String {
        let nodes = parse_nodes(src).unwrap();
        eval_nodes(&nodes, ctx)
    }

    #[test]
    fn resolves_placeholder() {
        let ctx = RenderContext::new().set("name", "Alice");
        assert_eq!(render("Hello {name}!", &ctx), "Hello Alice!");
    }

    #[test]
    fn applies_default_when_missing() {
        let ctx = RenderContext::new();
        assert_eq!(render("{name|default:Unknown}", &ctx), "Unknown");
    }

    #[test]
    fn default_not_applied_when_present() {
        let ctx = RenderContext::new().set("name", "Bob");
        assert_eq!(render("{name|default:Unknown}", &ctx), "Bob");
    }

    #[test]
    fn conditional_shows_when_key_present() {
        let ctx = RenderContext::new().set("in_party", "true");
        assert_eq!(render("{if:in_party}yes{/if}", &ctx), "yes");
    }

    #[test]
    fn conditional_hides_when_key_absent() {
        let ctx = RenderContext::new();
        assert_eq!(render("{if:in_party}yes{/if}", &ctx), "");
    }

    #[test]
    fn negated_conditional() {
        let ctx = RenderContext::new();
        assert_eq!(render("{if:!in_queue}not queued{/if}", &ctx), "not queued");
    }

    #[test]
    fn join_modifier_on_list() {
        let ctx = RenderContext::new().set("friends", vec!["Alice", "Bob"]);
        assert_eq!(render("{friends|join:, }", &ctx), "Alice, Bob");
    }

    #[test]
    fn plural_suffix_mode() {
        let ctx2 = RenderContext::new().set("count", 2i64);
        let ctx1 = RenderContext::new().set("count", 1i64);
        assert_eq!(render("{count} friend{count|plural::s}", &ctx1), "1 friend");
        assert_eq!(render("{count} friend{count|plural::s}", &ctx2), "2 friends");
    }

    #[test]
    fn key_ref_resolved_via_lookup() {
        let ctx = RenderContext::new();
        let nodes = parse_nodes("{key:staff.prefix}Hello").unwrap();
        let out = eval_nodes_full(&nodes, &ctx, &|k| {
            if k == "staff.prefix" { Some("[Staff] ".into()) } else { None }
        }, None);
        assert_eq!(out, "[Staff] Hello");
    }

    #[test]
    fn style_block_applies_registry_style() {
        let reg = StyleRegistry::new();
        reg.register("error", Style::new().color("#FF5555").bold());
        let ctx = RenderContext::new();
        let nodes = parse_nodes("{style:error}oops{/style}").unwrap();
        let out = eval_nodes_full(&nodes, &ctx, &|_| None, Some(&reg));
        assert!(out.contains("<color:#FF5555>"), "expected hex color tag, got: {out}");
        assert!(out.contains("<bold>"));
        assert!(out.contains("oops"));
        assert!(out.contains("<reset>"));
    }

    #[test]
    fn style_block_falls_through_without_registry() {
        let ctx = RenderContext::new();
        let nodes = parse_nodes("{style:error}oops{/style}").unwrap();
        let out = eval_nodes(&nodes, &ctx);
        assert_eq!(out, "oops");
    }

    #[test]
    fn gradient_block_emits_mini_message() {
        let ctx = RenderContext::new();
        let out = render("{gradient:#FF0000:#0000FF}Rainbow{/gradient}", &ctx);
        assert!(out.contains("<gradient:#FF0000:#0000FF>"));
        assert!(out.contains("Rainbow"));
    }

    #[test]
    fn hex_block_emits_mini_message() {
        let ctx = RenderContext::new();
        let out = render("{#FF5500}Orange{/}", &ctx);
        assert!(out.contains("<#FF5500>"));
        assert!(out.contains("Orange"));
    }
}
