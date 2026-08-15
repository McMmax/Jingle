use std::collections::HashMap;
use std::time::Duration;

const PERMANENT_SECS: u64 = 30 * 24 * 3600; // 30 days threshold

/// a typed value that can be injected into a template placeholder
pub enum PlaceholderValue {
    Text(String),
    Int(i64),
    Float(f64),
    Dur(Duration),
    List(Vec<String>),
    Lazy(Box<dyn Fn() -> String + Send + Sync>),
}

impl PlaceholderValue {
    /// resolves to a display string — lazy variants are evaluated once here
    pub fn as_str_owned(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Int(n) => n.to_string(),
            Self::Float(f) => format!("{f:.2}"),
            Self::Dur(d) => format_duration(*d),
            Self::List(items) => items.join(", "),
            Self::Lazy(f) => f(),
        }
    }

    pub fn as_list(&self) -> Option<&[String]> {
        if let Self::List(v) = self { Some(v) } else { None }
    }
}

impl std::fmt::Debug for PlaceholderValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text(s) => write!(f, "Text({s:?})"),
            Self::Int(n) => write!(f, "Int({n})"),
            Self::Float(n) => write!(f, "Float({n})"),
            Self::Dur(d) => write!(f, "Dur({d:?})"),
            Self::List(v) => write!(f, "List({v:?})"),
            Self::Lazy(_) => write!(f, "Lazy(...)"),
        }
    }
}

/// context passed to a render call — holds all typed placeholder values
#[derive(Debug, Default)]
pub struct RenderContext {
    values: HashMap<String, PlaceholderValue>,
}

impl RenderContext {
    pub fn new() -> Self {
        Self { values: HashMap::with_capacity(8) }
    }

    pub fn set<K, V>(mut self, key: K, val: V) -> Self
    where
        K: Into<String>,
        V: IntoPlaceholder,
    {
        self.values.insert(key.into(), val.into_placeholder());
        self
    }

    pub fn lazy<K, F>(mut self, key: K, f: F) -> Self
    where
        K: Into<String>,
        F: Fn() -> String + Send + Sync + 'static,
    {
        self.values.insert(key.into(), PlaceholderValue::Lazy(Box::new(f)));
        self
    }

    pub fn get(&self, key: &str) -> Option<&PlaceholderValue> {
        self.values.get(key)
    }

    pub fn resolve(&self, key: &str) -> Option<String> {
        self.values.get(key).map(|v| v.as_str_owned())
    }

    pub fn contains(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }
}

/// sealed conversion into a placeholder value — implemented for common types
pub trait IntoPlaceholder {
    fn into_placeholder(self) -> PlaceholderValue;
}

impl IntoPlaceholder for String {
    fn into_placeholder(self) -> PlaceholderValue { PlaceholderValue::Text(self) }
}

impl IntoPlaceholder for &str {
    fn into_placeholder(self) -> PlaceholderValue { PlaceholderValue::Text(self.to_owned()) }
}

impl IntoPlaceholder for i64 {
    fn into_placeholder(self) -> PlaceholderValue { PlaceholderValue::Int(self) }
}

impl IntoPlaceholder for i32 {
    fn into_placeholder(self) -> PlaceholderValue { PlaceholderValue::Int(self as i64) }
}

impl IntoPlaceholder for u32 {
    fn into_placeholder(self) -> PlaceholderValue { PlaceholderValue::Int(self as i64) }
}

impl IntoPlaceholder for u64 {
    fn into_placeholder(self) -> PlaceholderValue { PlaceholderValue::Int(self as i64) }
}

impl IntoPlaceholder for f64 {
    fn into_placeholder(self) -> PlaceholderValue { PlaceholderValue::Float(self) }
}

impl IntoPlaceholder for Duration {
    fn into_placeholder(self) -> PlaceholderValue { PlaceholderValue::Dur(self) }
}

impl IntoPlaceholder for Vec<String> {
    fn into_placeholder(self) -> PlaceholderValue { PlaceholderValue::List(self) }
}

impl IntoPlaceholder for Vec<&str> {
    fn into_placeholder(self) -> PlaceholderValue {
        PlaceholderValue::List(self.into_iter().map(str::to_owned).collect())
    }
}

pub(crate) fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    if total == 0 { return "0 seconds".into(); }
    if total >= PERMANENT_SECS { return "permanent".into(); }

    let hours = total / 3600;
    let mins  = (total % 3600) / 60;
    let secs  = total % 60;

    let mut parts = Vec::with_capacity(3);
    if hours > 0 { parts.push(format!("{hours} hour{}", if hours == 1 { "" } else { "s" })); }
    if mins  > 0 { parts.push(format!("{mins} minute{}", if mins == 1 { "" } else { "s" })); }
    if secs  > 0 { parts.push(format!("{secs} second{}", if secs == 1 { "" } else { "s" })); }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_resolves_string() {
        let ctx = RenderContext::new().set("name", "Alice");
        assert_eq!(ctx.resolve("name").unwrap(), "Alice");
    }

    #[test]
    fn context_resolves_u32() {
        let ctx = RenderContext::new().set("count", 42u32);
        assert_eq!(ctx.resolve("count").unwrap(), "42");
    }

    #[test]
    fn context_resolves_duration() {
        let ctx = RenderContext::new().set("time", Duration::from_secs(65));
        assert_eq!(ctx.resolve("time").unwrap(), "1 minute 5 seconds");
    }

    #[test]
    fn duration_permanent_threshold() {
        let ctx = RenderContext::new().set("ban", Duration::from_secs(PERMANENT_SECS));
        assert_eq!(ctx.resolve("ban").unwrap(), "permanent");
    }

    #[test]
    fn duration_just_below_threshold() {
        let ctx = RenderContext::new().set("t", Duration::from_secs(PERMANENT_SECS - 1));
        assert_ne!(ctx.resolve("t").unwrap(), "permanent");
    }

    #[test]
    fn duration_zero() {
        let ctx = RenderContext::new().set("t", Duration::ZERO);
        assert_eq!(ctx.resolve("t").unwrap(), "0 seconds");
    }

    #[test]
    fn lazy_evaluated_on_resolve() {
        let ctx = RenderContext::new().lazy("x", || "computed".into());
        assert_eq!(ctx.resolve("x").unwrap(), "computed");
    }

    #[test]
    fn contains_returns_false_for_missing() {
        let ctx = RenderContext::new();
        assert!(!ctx.contains("nope"));
    }
}
