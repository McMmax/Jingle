//! message keys, a small template language, styling, and a render pipeline.
//!
//! a catalog holds [`MessageKey`] entries keyed by dotted path. text is written
//! in the template language ([`Template`]) and resolved against a [`RenderContext`]
//! of typed placeholder values. a [`Renderer`] turns a key plus a context into a
//! [`RenderedMessage`] for a given [`OutputTarget`], applying middleware on the way out.
//!
//! style blocks (`{style:name}...{/style}`) are resolved against the [`StyleRegistry`]
//! held on the catalog. register presets at startup, then render with full formatting.

pub mod catalog;
pub mod compose;
pub mod entry_block;
pub mod key;
pub mod macros;
pub mod placeholder;
pub mod render;
pub mod style;
pub mod template;
pub mod variants;

#[cfg(feature = "dev")]
pub mod dev;

pub use catalog::{Catalog, MessageStore, NoopStore};
pub use compose::{MsgChain, MsgSequence};
pub use entry_block::{Condition, EntryBlock, MessageDefaultEntry, Priority};
pub use key::MessageKey;
pub use placeholder::{PlaceholderValue, RenderContext};
pub use render::{OutputTarget, RenderMiddleware, RenderedMessage, Renderer};
pub use style::{ColorSpec, Style, StyleRegistry};
pub use template::{Template, TemplateError};
pub use variants::{Tier, VariantSelection};
