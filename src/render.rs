use crate::catalog::Catalog;
use crate::placeholder::RenderContext;
use crate::style::{strip_all_formatting, strip_legacy_codes, strip_mini_message_tags};

/// output surface — each target may transform the text differently
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputTarget {
    Chat,
    ActionBar,
    Title,
    BossBar,
    TabList,
    Kick,
    Console,
}

/// the result of a render call
#[derive(Debug, Clone)]
pub struct RenderedMessage {
    pub text: String,
    pub target: OutputTarget,
    /// for Title target, the text split at `||` into (title, subtitle)
    pub title_parts: Option<(String, String)>,
}

impl RenderedMessage {
    fn new(text: String, target: OutputTarget) -> Self {
        let title_parts = if target == OutputTarget::Title {
            Some(split_title(&text))
        } else {
            None
        };
        Self { text, target, title_parts }
    }
}

fn split_title(text: &str) -> (String, String) {
    match text.split_once("||") {
        Some((t, s)) => (t.to_owned(), s.to_owned()),
        None => (text.to_owned(), String::new()),
    }
}

/// a transform applied to rendered text before delivery
pub trait RenderMiddleware: Send + Sync {
    fn transform(&self, text: &str, target: OutputTarget) -> String;
}

/// strips all & legacy color codes regardless of target
pub struct StripLegacyCodes;
impl RenderMiddleware for StripLegacyCodes {
    fn transform(&self, text: &str, _target: OutputTarget) -> String {
        strip_legacy_codes(text)
    }
}

/// strips & codes only when target is Console
pub struct ConsoleStrip;
impl RenderMiddleware for ConsoleStrip {
    fn transform(&self, text: &str, target: OutputTarget) -> String {
        if target == OutputTarget::Console {
            strip_legacy_codes(text)
        } else {
            text.to_owned()
        }
    }
}

/// strips MiniMessage tags only
pub struct StripMiniMessage;
impl RenderMiddleware for StripMiniMessage {
    fn transform(&self, text: &str, _target: OutputTarget) -> String {
        strip_mini_message_tags(text)
    }
}

/// strips both & codes and MiniMessage tags — ideal for Console or log output
pub struct StripAllFormatting;
impl RenderMiddleware for StripAllFormatting {
    fn transform(&self, text: &str, _target: OutputTarget) -> String {
        strip_all_formatting(text)
    }
}

/// strips all formatting but only when target is Console
pub struct ConsoleFullStrip;
impl RenderMiddleware for ConsoleFullStrip {
    fn transform(&self, text: &str, target: OutputTarget) -> String {
        if target == OutputTarget::Console {
            strip_all_formatting(text)
        } else {
            text.to_owned()
        }
    }
}

/// appends a fixed suffix to every message
pub struct GlobalSuffixAppender(pub String);
impl RenderMiddleware for GlobalSuffixAppender {
    fn transform(&self, text: &str, _target: OutputTarget) -> String {
        format!("{text}{}", self.0)
    }
}

/// strips newlines for targets that don't support them (ActionBar, BossBar)
pub struct ActionBarSanitize;
impl RenderMiddleware for ActionBarSanitize {
    fn transform(&self, text: &str, target: OutputTarget) -> String {
        if matches!(target, OutputTarget::ActionBar | OutputTarget::BossBar) {
            text.replace('\n', " ")
        } else {
            text.to_owned()
        }
    }
}

/// render pipeline — built once, applied to every render call
pub struct Renderer {
    middleware: Vec<Box<dyn RenderMiddleware>>,
}

impl Renderer {
    pub fn builder() -> RendererBuilder {
        RendererBuilder { middleware: Vec::new() }
    }

    /// bare renderer with no middleware — text passes through unchanged
    pub fn plain() -> Self {
        Self { middleware: Vec::new() }
    }

    pub fn render(
        &self,
        catalog: &Catalog,
        key: &str,
        target: OutputTarget,
        ctx: &RenderContext,
    ) -> Option<RenderedMessage> {
        let raw = catalog.render(key, ctx)?;
        let text = self.apply_middleware(&raw, target);
        Some(RenderedMessage::new(text, target))
    }

    fn apply_middleware(&self, text: &str, target: OutputTarget) -> String {
        // PERF(max): fold avoids allocating intermediate vecs; each middleware may alloc
        self.middleware.iter().fold(text.to_owned(), |t, m| m.transform(&t, target))
    }
}

pub struct RendererBuilder {
    middleware: Vec<Box<dyn RenderMiddleware>>,
}

impl RendererBuilder {
    pub fn middleware(mut self, m: impl RenderMiddleware + 'static) -> Self {
        self.middleware.push(Box::new(m));
        self
    }

    pub fn build(self) -> Renderer {
        Renderer { middleware: self.middleware }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::key::MessageKey;
    use crate::placeholder::RenderContext;
    use crate::style::{Style, StyleRegistry};

    fn catalog_with(key: &'static str, val: &'static str) -> Catalog {
        let mut cat = Catalog::new();
        cat.register(MessageKey::new(key, val));
        cat
    }

    #[test]
    fn plain_render() {
        let cat = catalog_with("greet", "Hello {name}!");
        let ctx = RenderContext::new().set("name", "Bob");
        let r = Renderer::plain().render(&cat, "greet", OutputTarget::Chat, &ctx).unwrap();
        assert_eq!(r.text, "Hello Bob!");
    }

    #[test]
    fn console_strip_middleware() {
        let cat = catalog_with("msg", "&aGreen text");
        let ctx = RenderContext::new();
        let renderer = Renderer::builder().middleware(ConsoleStrip).build();
        let r = renderer.render(&cat, "msg", OutputTarget::Console, &ctx).unwrap();
        assert_eq!(r.text, "Green text");
    }

    #[test]
    fn console_strip_leaves_chat_intact() {
        let cat = catalog_with("msg", "&aGreen text");
        let ctx = RenderContext::new();
        let renderer = Renderer::builder().middleware(ConsoleStrip).build();
        let r = renderer.render(&cat, "msg", OutputTarget::Chat, &ctx).unwrap();
        assert_eq!(r.text, "&aGreen text");
    }

    #[test]
    fn strip_all_formatting_removes_both() {
        let cat = catalog_with("msg", "&a<red>text</red>");
        let ctx = RenderContext::new();
        let renderer = Renderer::builder().middleware(StripAllFormatting).build();
        let r = renderer.render(&cat, "msg", OutputTarget::Chat, &ctx).unwrap();
        assert_eq!(r.text, "text");
    }

    #[test]
    fn title_splits_at_double_pipe() {
        let cat = catalog_with("title", "Main Title||Sub Title");
        let ctx = RenderContext::new();
        let r = Renderer::plain().render(&cat, "title", OutputTarget::Title, &ctx).unwrap();
        let (t, s) = r.title_parts.unwrap();
        assert_eq!(t, "Main Title");
        assert_eq!(s, "Sub Title");
    }

    #[test]
    fn suffix_appender_adds_reset() {
        let cat = catalog_with("msg", "text");
        let ctx = RenderContext::new();
        let renderer = Renderer::builder()
            .middleware(GlobalSuffixAppender("&r".into()))
            .build();
        let r = renderer.render(&cat, "msg", OutputTarget::Chat, &ctx).unwrap();
        assert_eq!(r.text, "text&r");
    }

    #[test]
    fn action_bar_sanitize_removes_newlines() {
        let cat = catalog_with("msg", "line1\nline2");
        let renderer = Renderer::builder().middleware(ActionBarSanitize).build();
        let r = renderer.render(&cat, "msg", OutputTarget::ActionBar, &RenderContext::new()).unwrap();
        assert!(!r.text.contains('\n'));
    }

    #[test]
    fn render_returns_none_for_missing_key() {
        let cat = Catalog::new();
        let r = Renderer::plain().render(&cat, "nope", OutputTarget::Chat, &RenderContext::new());
        assert!(r.is_none());
    }

    #[test]
    fn style_block_rendered_via_catalog_registry() {
        let reg = StyleRegistry::new();
        reg.register("warn", Style::new().color("gold").bold());
        let mut cat = Catalog::new().with_style_registry(reg);
        cat.register(MessageKey::new("alert", "{style:warn}Warning!{/style}"));
        let r = Renderer::plain().render(&cat, "alert", OutputTarget::Chat, &RenderContext::new()).unwrap();
        assert!(r.text.contains("Warning!"));
        assert!(r.text.contains("<bold>"), "expected bold tag in: {}", r.text);
    }

    #[test]
    fn console_full_strip_removes_mini_message() {
        let reg = StyleRegistry::new();
        reg.register("err", Style::new().color("#FF5555").bold());
        let mut cat = Catalog::new().with_style_registry(reg);
        cat.register(MessageKey::new("msg", "{style:err}Error!{/style}"));
        let renderer = Renderer::builder().middleware(ConsoleFullStrip).build();
        let r = renderer.render(&cat, "msg", OutputTarget::Console, &RenderContext::new()).unwrap();
        assert!(!r.text.contains('<'), "MiniMessage tags not stripped: {}", r.text);
        assert!(r.text.contains("Error!"));
    }
}
