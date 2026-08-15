use std::collections::HashMap;

/// flat entry for SpacetimeDB `sync_message_defaults`
#[derive(Debug, Clone, PartialEq)]
pub struct MessageDefaultEntry {
    pub key: String,
    pub value: String,
}

/// priority level for conflict resolution when merging entry blocks
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Admin = 0,
    Plugin = 1,
    Core = 2,
}

/// condition under which entries in this block are applied
#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    Always,
    KeyAbsent,
    VersionLessThan(String),
}

/// a named, prioritized, namespaced batch of message defaults
#[derive(Debug, Clone)]
pub struct EntryBlock {
    id: String,
    priority: Priority,
    namespace: Option<String>,
    condition: Condition,
    tag: Option<String>,
    entries: Vec<(String, String)>,
}

impl EntryBlock {
    pub fn named(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            priority: Priority::Plugin,
            namespace: None,
            condition: Condition::Always,
            tag: None,
            entries: Vec::with_capacity(32),
        }
    }

    pub fn priority(mut self, p: Priority) -> Self {
        self.priority = p;
        self
    }

    /// all keys in this block are prefixed with `{ns}.`
    pub fn namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = Some(ns.into());
        self
    }

    pub fn entry(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.entries.push((key.into(), value.into()));
        self
    }

    pub fn conditional(mut self, c: Condition) -> Self {
        self.condition = c;
        self
    }

    pub fn tag(mut self, t: impl Into<String>) -> Self {
        self.tag = Some(t.into());
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn get_priority(&self) -> Priority {
        self.priority
    }

    pub fn condition(&self) -> &Condition {
        &self.condition
    }

    pub fn block_tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    /// resolves full dotted keys and emits flat entries for SpacetimeDB
    pub fn to_entries(&self) -> Vec<MessageDefaultEntry> {
        self.entries
            .iter()
            .map(|(k, v)| {
                let key = if let Some(ns) = &self.namespace {
                    format!("{ns}.{k}")
                } else {
                    k.clone()
                };
                MessageDefaultEntry { key, value: v.clone() }
            })
            .collect()
    }

    /// merges blocks, higher priority wins on key collision
    pub fn merge(blocks: Vec<EntryBlock>) -> Vec<MessageDefaultEntry> {
        // PERF(max): single pass with priority comparison, no sort needed
        let mut map: HashMap<String, (Priority, String)> = HashMap::with_capacity(
            blocks.iter().map(|b| b.entries.len()).sum()
        );

        for block in &blocks {
            for entry in block.to_entries() {
                let current_priority = map.get(&entry.key).map(|(p, _)| *p);
                match current_priority {
                    None => { map.insert(entry.key, (block.priority, entry.value)); }
                    Some(existing) if block.priority > existing => {
                        map.insert(entry.key, (block.priority, entry.value));
                    }
                    _ => {} // existing entry wins
                }
            }
        }

        let mut result: Vec<MessageDefaultEntry> = map
            .into_iter()
            .map(|(key, (_, value))| MessageDefaultEntry { key, value })
            .collect();
        result.sort_by(|a, b| a.key.cmp(&b.key)); // stable ordering for SpacetimeDB upserts
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_entries_applies_namespace() {
        let block = EntryBlock::named("test")
            .namespace("staff")
            .entry("prefix", "&3[Staff] ");
        let entries = block.to_entries();
        assert_eq!(entries[0].key, "staff.prefix");
        assert_eq!(entries[0].value, "&3[Staff] ");
    }

    #[test]
    fn merge_core_wins_over_plugin() {
        let plugin = EntryBlock::named("plugin")
            .priority(Priority::Plugin)
            .entry("staff.prefix", "&7[Plugin] ");
        let core = EntryBlock::named("core")
            .priority(Priority::Core)
            .entry("staff.prefix", "&3[Core] ");
        let merged = EntryBlock::merge(vec![plugin, core]);
        let entry = merged.iter().find(|e| e.key == "staff.prefix").unwrap();
        assert_eq!(entry.value, "&3[Core] ");
    }

    #[test]
    fn merge_deduplicates_keys() {
        let a = EntryBlock::named("a").priority(Priority::Plugin).entry("foo", "1");
        let b = EntryBlock::named("b").priority(Priority::Plugin).entry("foo", "2");
        let merged = EntryBlock::merge(vec![a, b]);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merge_preserves_unique_keys() {
        let a = EntryBlock::named("a").entry("foo", "1");
        let b = EntryBlock::named("b").entry("bar", "2");
        let merged = EntryBlock::merge(vec![a, b]);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_output_is_sorted() {
        let block = EntryBlock::named("x")
            .entry("z.key", "z")
            .entry("a.key", "a");
        let merged = EntryBlock::merge(vec![block]);
        assert_eq!(merged[0].key, "a.key");
        assert_eq!(merged[1].key, "z.key");
    }
}
