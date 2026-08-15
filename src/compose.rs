use std::time::Duration;

/// concatenates message keys at render time with a separator
#[derive(Debug, Clone)]
pub struct MsgChain {
    keys: Vec<&'static str>,
    separator: String,
}

impl MsgChain {
    pub fn of(key: &'static str) -> Self {
        Self { keys: vec![key], separator: String::new() }
    }

    pub fn then(mut self, key: &'static str) -> Self {
        self.keys.push(key);
        self
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn keys(&self) -> &[&'static str] {
        &self.keys
    }

    pub fn separator_str(&self) -> &str {
        &self.separator
    }

    /// resolves all keys via a lookup closure and joins them
    pub fn render<F>(&self, lookup: F) -> String
    where
        F: Fn(&str) -> Option<String>,
    {
        self.keys
            .iter()
            .filter_map(|k| lookup(k))
            .collect::<Vec<_>>()
            .join(&self.separator)
    }
}

/// a time-ordered sequence of message keys for multi-part delivery
#[derive(Debug, Clone)]
pub struct MsgSequence {
    steps: Vec<SequenceStep>,
}

#[derive(Debug, Clone)]
pub struct SequenceStep {
    pub delay: Duration,
    pub key: &'static str,
}

impl MsgSequence {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn after(mut self, delay: Duration, key: &'static str) -> Self {
        self.steps.push(SequenceStep { delay, key });
        self
    }

    pub fn steps(&self) -> &[SequenceStep] {
        &self.steps
    }
}

impl Default for MsgSequence {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_renders_with_separator() {
        let chain = MsgChain::of("a").then("b").separator(" | ");
        let out = chain.render(|k| Some(format!("[{k}]")));
        assert_eq!(out, "[a] | [b]");
    }

    #[test]
    fn chain_skips_missing_keys() {
        let chain = MsgChain::of("a").then("missing").then("c");
        let out = chain.render(|k| {
            if k == "missing" { None } else { Some(k.to_owned()) }
        });
        assert_eq!(out, "ac");
    }

    #[test]
    fn sequence_preserves_order_and_delay() {
        let seq = MsgSequence::new()
            .after(Duration::ZERO, "welcome.title")
            .after(Duration::from_secs(2), "welcome.subtitle");
        assert_eq!(seq.steps()[0].key, "welcome.title");
        assert_eq!(seq.steps()[1].delay, Duration::from_secs(2));
    }
}
