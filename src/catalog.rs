use crate::entry_block::EntryBlock;
use crate::key::MessageKey;
use crate::placeholder::RenderContext;
use crate::style::StyleRegistry;
use crate::template::Template;
use crate::variants::VariantSelection;
use std::collections::HashMap;
use std::sync::Mutex;

/// store adapter — implemented by SpacetimeDB Cache in Internode
pub trait MessageStore: Send + Sync {
    /// returns an admin-level text override for a key, if one exists
    fn get_override(&self, key: &str) -> Option<String>;
    /// returns the registered default text for a key
    fn get_default(&self, key: &str) -> Option<String>;
    /// returns all keys that have admin overrides — used by list_overridden
    fn list_overrides(&self) -> Vec<String> { Vec::new() }
}

/// no-op store — used when no SpacetimeDB connection is available
pub struct NoopStore;
impl MessageStore for NoopStore {
    fn get_override(&self, _key: &str) -> Option<String> { None }
    fn get_default(&self, _key: &str) -> Option<String> { None }
}

/// catalog of all registered message keys with namespace indexing
pub struct Catalog {
    keys: HashMap<String, MessageKey>,
    /// index: namespace prefix → list of full dotted keys
    ns_index: HashMap<String, Vec<String>>,
    store: Box<dyn MessageStore>,
    /// style registry for {style:name} blocks in templates
    style_registry: StyleRegistry,
    /// callbacks fired by notify_override — registered at startup
    override_listeners: Vec<Box<dyn Fn(&str, Option<&str>, &str) + Send + Sync + 'static>>,
    /// per-key round-robin counters — interior mutable so render() takes &self
    round_robin: Mutex<HashMap<String, usize>>,
    /// dev-only: set of keys that have had render() called at least once
    #[cfg(feature = "dev")]
    tracking: Option<Mutex<std::collections::HashSet<String>>>,
}

impl Catalog {
    pub fn new() -> Self {
        Self {
            keys: HashMap::with_capacity(256),
            ns_index: HashMap::with_capacity(32),
            store: Box::new(NoopStore),
            style_registry: StyleRegistry::new(),
            override_listeners: Vec::new(),
            round_robin: Mutex::new(HashMap::with_capacity(16)),
            #[cfg(feature = "dev")]
            tracking: None,
        }
    }

    pub fn with_store(mut self, store: impl MessageStore + 'static) -> Self {
        self.store = Box::new(store);
        self
    }

    pub fn with_style_registry(mut self, reg: StyleRegistry) -> Self {
        self.style_registry = reg;
        self
    }

    pub fn style_registry(&self) -> &StyleRegistry {
        &self.style_registry
    }

    pub fn register(&mut self, key: MessageKey) {
        let k = key.key;
        let parts: Vec<&str> = k.split('.').collect();
        for i in 0..parts.len().saturating_sub(1) {
            let ns: String = parts[..=i].join(".");
            self.ns_index.entry(ns).or_insert_with(Vec::new).push(k.to_owned());
        }
        self.keys.insert(k.to_owned(), key);
    }

    pub fn register_all(&mut self, keys: impl IntoIterator<Item = MessageKey>) {
        for k in keys { self.register(k); }
    }

    pub fn get(&self, key: &str) -> Option<&MessageKey> {
        self.keys.get(key)
    }

    /// resolves the text for a key — store override → store default → compiled-in default
    /// for RoundRobin keys, rotates through variants on each call
    pub fn resolve_text(&self, key: &str) -> Option<String> {
        if let Some(ov) = self.store.get_override(key) {
            return Some(ov);
        }
        if let Some(def) = self.store.get_default(key) {
            return Some(def);
        }
        let msg = self.keys.get(key)?;
        let text = if matches!(msg.selection, VariantSelection::RoundRobin) && !msg.variants.is_empty() {
            // PERF(max): lock only for the counter increment, not the render
            let mut rr = self.round_robin.lock().unwrap();
            let counter = rr.entry(key.to_owned()).or_insert(0);
            msg.selection.pick_round_robin(msg.default, &msg.variants, counter)
        } else {
            msg.resolve()
        };
        Some(text.to_owned())
    }

    /// renders a key to a string against the given context — uses style registry internally
    pub fn render(&self, key: &str, ctx: &RenderContext) -> Option<String> {
        let text = self.resolve_text(key)?;
        let tmpl = Template::parse(&text).ok()?;
        let catalog_lookup = |k: &str| self.resolve_text(k);

        #[cfg(feature = "dev")]
        if let Some(ref tracking) = self.tracking {
            tracking.lock().unwrap().insert(key.to_owned());
        }

        Some(tmpl.render_full(ctx, catalog_lookup, Some(&self.style_registry)))
    }

    pub fn list_namespace(&self, ns: &str) -> Vec<&str> {
        self.ns_index
            .get(ns)
            .map(|v| v.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    /// returns keys that have an active store override — requires store to implement list_overrides
    pub fn list_overridden(&self) -> Vec<String> {
        self.store.list_overrides()
    }

    pub fn all_keys(&self) -> impl Iterator<Item = &str> {
        self.keys.keys().map(String::as_str)
    }

    /// converts registered keys into an EntryBlock for SpacetimeDB sync
    pub fn build_entry_block(&self, id: &str) -> EntryBlock {
        let mut block = EntryBlock::named(id);
        for (k, msg) in &self.keys {
            block = block.entry(k.as_str(), msg.default);
        }
        block
    }

    pub fn entry_count(&self) -> usize {
        self.keys.len()
    }

    /// registers a callback fired whenever notify_override is called
    pub fn on_override<F>(&mut self, cb: F)
    where
        F: Fn(&str, Option<&str>, &str) + Send + Sync + 'static,
    {
        self.override_listeners.push(Box::new(cb));
    }

    /// fires all registered override callbacks — call from Internode when SpacetimeDB updates
    pub fn notify_override(&self, key: &str, old: Option<&str>, new_val: &str) {
        for cb in &self.override_listeners {
            cb(key, old, new_val);
        }
    }

    // --- dev-feature methods ---

    /// lints all registered templates — run at startup; returns issues found
    #[cfg(feature = "dev")]
    pub fn lint_all(&self) -> Vec<crate::dev::LintIssue> {
        crate::dev::lint_catalog(self)
    }

    /// enables render-call tracking — call once at startup; pairs with unused_keys
    #[cfg(feature = "dev")]
    pub fn enable_tracking(&mut self) {
        self.tracking = Some(Mutex::new(std::collections::HashSet::with_capacity(self.keys.len())));
    }

    /// returns keys defined in the catalog but never rendered since enable_tracking was called
    #[cfg(feature = "dev")]
    pub fn unused_keys(&self) -> Vec<&str> {
        if let Some(ref tracking) = self.tracking {
            let rendered = tracking.lock().unwrap();
            self.keys.keys()
                .filter(|k| !rendered.contains(*k))
                .map(String::as_str)
                .collect()
        } else {
            Vec::new()
        }
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Catalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Catalog({} keys)", self.keys.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_catalog() -> Catalog {
        let mut cat = Catalog::new();
        cat.register(MessageKey::new("staff.prefix", "&3[Staff] "));
        cat.register(MessageKey::new("friend.add.sent", "Request sent to {target}!"));
        cat
    }

    #[test]
    fn register_and_get() {
        let cat = make_catalog();
        assert!(cat.get("staff.prefix").is_some());
        assert!(cat.get("missing").is_none());
    }

    #[test]
    fn resolve_text_returns_compiled_default() {
        let cat = make_catalog();
        assert_eq!(cat.resolve_text("staff.prefix").unwrap(), "&3[Staff] ");
    }

    #[test]
    fn store_override_wins() {
        struct OverrideStore;
        impl MessageStore for OverrideStore {
            fn get_override(&self, key: &str) -> Option<String> {
                if key == "staff.prefix" { Some("&4[Admin] ".into()) } else { None }
            }
            fn get_default(&self, _key: &str) -> Option<String> { None }
        }
        let mut cat = Catalog::new().with_store(OverrideStore);
        cat.register(MessageKey::new("staff.prefix", "&3[Staff] "));
        assert_eq!(cat.resolve_text("staff.prefix").unwrap(), "&4[Admin] ");
    }

    #[test]
    fn render_substitutes_placeholder() {
        let cat = make_catalog();
        let ctx = RenderContext::new().set("target", "Alice");
        assert_eq!(cat.render("friend.add.sent", &ctx).unwrap(), "Request sent to Alice!");
    }

    #[test]
    fn namespace_index_lists_keys() {
        let cat = make_catalog();
        let keys = cat.list_namespace("staff");
        assert!(keys.contains(&"staff.prefix"));
    }

    #[test]
    fn build_entry_block_includes_all_keys() {
        let cat = make_catalog();
        let block = cat.build_entry_block("test");
        let entries = block.to_entries();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn round_robin_rotates() {
        let mut cat = Catalog::new();
        cat.register(
            MessageKey::new("motd", "default")
                .variant("a")
                .variant("b")
                .variant("c")
                .selection(VariantSelection::RoundRobin)
        );
        let r0 = cat.resolve_text("motd").unwrap();
        let r1 = cat.resolve_text("motd").unwrap();
        let r2 = cat.resolve_text("motd").unwrap();
        let r3 = cat.resolve_text("motd").unwrap();
        assert_eq!(r0, "a");
        assert_eq!(r1, "b");
        assert_eq!(r2, "c");
        assert_eq!(r3, "a"); // wraps
    }

    #[test]
    fn notify_override_fires_callback() {
        use std::sync::{Arc, Mutex};
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let log2 = log.clone();
        let mut cat = Catalog::new();
        cat.on_override(move |key, old, new| {
            log2.lock().unwrap().push(format!("{key}:{old:?}->{new}"));
        });
        cat.notify_override("staff.prefix", Some("&3[Staff] "), "&4[Admin] ");
        let entries = log.lock().unwrap();
        assert_eq!(entries[0], r#"staff.prefix:Some("&3[Staff] ")->&4[Admin] "#);
    }

    #[test]
    fn list_overridden_returns_store_overrides() {
        struct TrackingStore { keys: Vec<String> }
        impl MessageStore for TrackingStore {
            fn get_override(&self, key: &str) -> Option<String> {
                if self.keys.contains(&key.to_owned()) { Some("override".into()) } else { None }
            }
            fn get_default(&self, _key: &str) -> Option<String> { None }
            fn list_overrides(&self) -> Vec<String> { self.keys.clone() }
        }
        let cat = Catalog::new().with_store(TrackingStore {
            keys: vec!["staff.prefix".into()],
        });
        let overridden = cat.list_overridden();
        assert!(overridden.contains(&"staff.prefix".to_owned()));
    }

    #[cfg(feature = "dev")]
    #[test]
    fn unused_keys_tracks_renders() {
        let mut cat = make_catalog();
        cat.enable_tracking();
        let ctx = RenderContext::new().set("target", "Bob");
        cat.render("friend.add.sent", &ctx);
        let unused = cat.unused_keys();
        assert!(unused.contains(&"staff.prefix"), "staff.prefix not rendered, should be unused");
        assert!(!unused.contains(&"friend.add.sent"), "friend.add.sent was rendered");
    }
}
