use crate::variants::VariantSelection;

/// a single message key with optional variants and metadata
#[derive(Debug, Clone)]
pub struct MessageKey {
    pub key: &'static str,
    pub default: &'static str,
    pub variants: Vec<&'static str>,
    pub selection: VariantSelection,
}

impl MessageKey {
    pub const fn new(key: &'static str, default: &'static str) -> Self {
        Self {
            key,
            default,
            variants: Vec::new(),
            selection: VariantSelection::Fixed,
        }
    }

    pub fn variant(mut self, text: &'static str) -> Self {
        self.variants.push(text);
        self
    }

    pub fn selection(mut self, sel: VariantSelection) -> Self {
        self.selection = sel;
        self
    }

    /// resolves which template text to use, ignoring variant logic for Fixed
    pub fn resolve(&self) -> &'static str {
        self.selection.pick(self.default, &self.variants)
    }
}
