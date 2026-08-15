/// dev tools — gated behind the `dev` feature flag
///
/// accessible via free functions or via Catalog methods (lint_all, enable_tracking, unused_keys).
/// never enable in production builds.
use crate::catalog::Catalog;
use crate::template::Template;

/// a single lint finding
#[derive(Debug, Clone)]
pub struct LintIssue {
    pub key: String,
    pub kind: LintKind,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LintKind {
    EmptyDefault,
    ParseError,
    DuplicateKey,
    UnknownStyleRef,
}

impl std::fmt::Display for LintIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.detail {
            Some(d) => write!(f, "[{:?}] {}: {d}", self.kind, self.key),
            None    => write!(f, "[{:?}] {}", self.kind, self.key),
        }
    }
}

/// lints all templates in the catalog — run at startup, not on hot path
pub fn lint_catalog(catalog: &Catalog) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let style_names: std::collections::HashSet<String> =
        catalog.style_registry().names().into_iter().collect();

    for key in catalog.all_keys() {
        if !seen.insert(key.to_owned()) {
            issues.push(LintIssue {
                key: key.to_owned(),
                kind: LintKind::DuplicateKey,
                detail: None,
            });
            continue;
        }
        if let Some(text) = catalog.resolve_text(key) {
            if text.trim().is_empty() {
                issues.push(LintIssue { key: key.to_owned(), kind: LintKind::EmptyDefault, detail: None });
                continue;
            }
            match Template::parse(&text) {
                Err(e) => {
                    issues.push(LintIssue {
                        key: key.to_owned(),
                        kind: LintKind::ParseError,
                        detail: Some(e.to_string()),
                    });
                }
                Ok(tmpl) => {
                    // check for {style:name} refs to unregistered styles
                    collect_unknown_style_refs(&tmpl, &style_names, key, &mut issues);
                }
            }
        }
    }
    issues
}

fn collect_unknown_style_refs(
    tmpl: &crate::template::Template,
    known: &std::collections::HashSet<String>,
    key: &str,
    issues: &mut Vec<LintIssue>,
) {
    use crate::template::TemplateNode;
    for node in &tmpl.nodes {
        if let TemplateNode::StyledBlock { style, .. } = node {
            if !known.contains(style.as_str()) {
                issues.push(LintIssue {
                    key: key.to_owned(),
                    kind: LintKind::UnknownStyleRef,
                    detail: Some(format!("style '{style}' not registered")),
                });
            }
        }
    }
}

/// standalone key tracker — use Catalog::enable_tracking / Catalog::unused_keys in normal code
pub struct KeyTracker {
    rendered: std::sync::Mutex<std::collections::HashSet<String>>,
}

impl KeyTracker {
    pub fn new() -> Self {
        Self { rendered: std::sync::Mutex::new(std::collections::HashSet::new()) }
    }

    pub fn record(&self, key: &str) {
        self.rendered.lock().unwrap().insert(key.to_owned());
    }

    /// returns keys in catalog that were never rendered
    pub fn unused_keys<'a>(&self, catalog: &'a Catalog) -> Vec<&'a str> {
        let rendered = self.rendered.lock().unwrap();
        catalog.all_keys().filter(|k| !rendered.contains(*k)).collect()
    }
}

impl Default for KeyTracker {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Catalog;
    use crate::key::MessageKey;
    use crate::style::{Style, StyleRegistry};

    #[test]
    fn lint_catches_empty_default() {
        let mut cat = Catalog::new();
        cat.register(MessageKey::new("empty.key", ""));
        let issues = lint_catalog(&cat);
        assert!(issues.iter().any(|i| i.kind == LintKind::EmptyDefault));
    }

    #[test]
    fn lint_passes_valid_template() {
        let mut cat = Catalog::new();
        cat.register(MessageKey::new("valid.key", "Hello {name}!"));
        let issues = lint_catalog(&cat);
        assert!(issues.is_empty(), "unexpected issues: {issues:?}");
    }

    #[test]
    fn lint_catches_parse_error() {
        let mut cat = Catalog::new();
        cat.register(MessageKey::new("bad.key", "{unclosed"));
        let issues = lint_catalog(&cat);
        assert!(issues.iter().any(|i| i.kind == LintKind::ParseError));
    }

    #[test]
    fn lint_catches_unknown_style_ref() {
        let mut cat = Catalog::new();
        cat.register(MessageKey::new("styled.key", "{style:missing}text{/style}"));
        let issues = lint_catalog(&cat);
        assert!(issues.iter().any(|i| i.kind == LintKind::UnknownStyleRef));
    }

    #[test]
    fn lint_passes_known_style_ref() {
        let reg = StyleRegistry::new();
        reg.register("error", Style::new().color("#FF5555"));
        let mut cat = Catalog::new().with_style_registry(reg);
        cat.register(MessageKey::new("styled.key", "{style:error}text{/style}"));
        let issues = lint_catalog(&cat);
        assert!(!issues.iter().any(|i| i.kind == LintKind::UnknownStyleRef));
    }

    #[test]
    fn tracker_reports_unused() {
        let mut cat = Catalog::new();
        cat.register(MessageKey::new("used.key", "hello"));
        cat.register(MessageKey::new("unused.key", "world"));
        let tracker = KeyTracker::new();
        tracker.record("used.key");
        let unused = tracker.unused_keys(&cat);
        assert!(unused.contains(&"unused.key"));
        assert!(!unused.contains(&"used.key"));
    }
}
