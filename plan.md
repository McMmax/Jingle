# Plan: Jingle — Standalone Message Key Library

## Context

Internode currently embeds its message key system directly in `internode-core/src/commands/messages.rs`: 150+ `MessageDefinition` constants, a static `CATALOG`, and `render`/`render_for` helpers backed by Holster (placeholder resolution) and SpacetimeDB (admin overrides). The user wants this extracted into its own polished, showcase-quality **standalone repo** called **Jingle** — the same pattern as Holster (placeholders) and Acmd (commands).

**Goal**: Create the `jingle` crate in a new standalone `Jingle` repository. Both Internode and Stomzy will depend on it. Initial integration uses a local workspace path dep; will switch to a git dep once the repo is live and stable.

**Repo**: `McMmax/Jingle` — implement in a fresh working session pointed at this repo.  
**Crate name**: `jingle`  
**Dependency style**: `path = "../Jingle"` in Internode/Stomzy Cargo.toml initially.

### Trace hygiene rules (hard requirements)
- No "Co-Authored-By" lines in commits
- No tooling attributions in comments, doc comments, or commit bodies
- Commits follow MINEMARES style: imperative, lowercase, no punctuation
- Branch: `jingle/initial` or similar per the `{component}/{what}` convention

---

## Style Constraints (from MINEMARES style guide)

- **Latency first**: render hot path must be lock-free, zero-alloc where possible. `PlaceholderArgs` uses `SmallVec` to avoid heap allocation for ≤8 attrs. Template nodes are pre-compiled, not re-parsed at render time.
- **Naming**: `PascalCase` types/traits, `snake_case` functions/modules, `SCREAMING_SNAKE` constants.
- **Comments**: inline, terse, lowercase, no period. Only WHY, never WHAT. `PERF(max):`, `SAFE(max):` etc. tags with handle.
- **Doc comments**: terse rustdoc on all public API, no name-repeat, no filler.
- **Errors**: `thiserror` enums per domain (`TemplateParseError`, `KeystoneError`).
- **Collections**: pre-sized, access-pattern driven. `HashMap::with_capacity`, `Vec::with_capacity`, `SmallVec`.
- **No null**: `Option<T>` everywhere, never assume `Some`.
- **Dead code**: deleted, not commented out. Ghost code gets `DEAD(max):` tag.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        keystone                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │ Catalog  │  │ Template │  │ Render   │  │  Store   │  │
│  │ System   │  │ Language │  │ Pipeline │  │ Adapter  │  │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │ Entry    │  │ Namespace│  │ Variant  │  │  Dev     │  │
│  │ Blocks   │  │ & Keys   │  │ System   │  │  Tools   │  │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## Feature Design (to be finalized after planning agent completes)

### 1. Entry Block System (Enhanced)

The current entry block is a flat batch of `MessageDefaultEntry { key, value }` sent to SpacetimeDB as `sync_message_defaults`. Enhance to:

```rust
// Named, prioritized, namespaced entry blocks
let block = EntryBlock::named("internode-core")
    .priority(Priority::Core)          // Core > Plugin > Admin default
    .namespace("staff")                // auto-prefixes keys: "staff.prefix"
    .entry("prefix", "&3[Staff] ")
    .entry("chat_format", "&b{player}&f: {message}")
    .conditional(Condition::KeyAbsent) // only upsert missing keys
    .tag("v1.2");                      // version tag for migration

// Merge multiple blocks, resolve conflicts by priority
let merged = EntryBlock::merge(vec![core_block, plugin_block, admin_block]);
```

**Types:**
- `EntryBlock` — builder for a named batch of message defaults
- `Priority` — `Core` > `Plugin` > `Admin` (for conflict resolution in merges)
- `Condition` — `Always`, `KeyAbsent`, `VersionLessThan(&str)`
- `BlockTag` — arbitrary version/identity string for migration tracking

---

### 2. Template Language

Extend beyond `{placeholder}` with a mini-language:

```
// Conditionals
{if:in_party}You're in {party_name}!{/if}
{if:!in_queue}You're not in any queue.{/if}

// Pluralization
{friends} friend{friends|plural::s}
{friends|plural:friend:friends}

// Nested key references (embed another message)
{key:staff.prefix}Hello {username}!

// Fallback defaults
{username|default:Unknown Player}

// List formatting
{friend_names|join:, |fallback:nobody}

// Style presets (defined in the registry)
{style:error}Something went wrong!{/style}
{style:header}── Friend List ──{/style}

// Gradients and hex color
{gradient:#FF0000:#0000FF}Rainbow{/gradient}
{#FF5500}Orange text{/}
```

**Types:**
- `Template` — parsed, validated template tree (compile-time safe if used via macro)
- `TemplateNode` — enum: `Literal(String)`, `Placeholder(PlaceholderExpr)`, `Conditional(...)`, `StyledBlock(...)`, `KeyRef(String)`
- `PlaceholderExpr` — `{ key, modifiers: Vec<Modifier> }`
- `Modifier` — `Plural { singular, plural }`, `Default(String)`, `Join(String)`, `Style(String)`, `Gradient { from, to }`

---

### 3. Decoration / Style System

```rust
// Register named style presets
StyleRegistry::register("error",   Style::new().color("#FF5555").bold());
StyleRegistry::register("success", Style::new().color("#55FF55"));
StyleRegistry::register("header",  Style::new().color("#AAAAAA").strikethrough());
StyleRegistry::register("warn",    Style::new().color("gold"));

// Legacy & codes still work
// New: mini-message format
// New: gradient spans
// New: hex inline
```

**Types:**
- `Style` — builder: `.color()`, `.bold()`, `.italic()`, `.gradient(from, to)`
- `StyleRegistry` — global registry, thread-safe
- `ColorSpec` — `Named(&str)`, `Hex(u32)`, `Legacy(char)`

---

### 4. Key Namespace & Organization

```rust
// Declare namespaces with a macro
msg_keys! {
    pub mod staff {
        PREFIX = "staff.prefix";
        CHAT_FORMAT = "staff.chat_format";
        pub mod freeze {
            FROZEN = "staff.freeze.frozen";
            UNFROZEN = "staff.freeze.unfrozen";
        }
    }
    pub mod friend {
        pub mod add {
            REQUEST_SENT = "friend.add.request_sent";
        }
    }
}

// Type-safe key paths (auto-generated consts)
let key: &'static str = keys::staff::PREFIX;         // "staff.prefix"
let key: &'static str = keys::staff::freeze::FROZEN;  // "staff.freeze.frozen"

// Key sets for bulk operations
let friend_keys: KeySet = KeySet::namespace("friend");
catalog.disable_namespace("friend"); // silence all friend messages
```

---

### 5. Render Pipeline

```rust
// Define output targets with different rendering rules
let renderer = Renderer::builder()
    .middleware(StripColorForConsole)    // strips & codes in console output
    .middleware(GlobalSuffixAppender("&r".into()))
    .middleware(RateLimiter::per_player(5, Duration::from_secs(1)))
    .build();

// Render to a specific output target
let rendered: RenderedMessage = renderer.render(
    &catalog,
    keys::FRIEND_ADD_REQUEST_SENT,
    OutputTarget::Chat,
    ctx.with("target", "Alice"),
)?;

// Target-aware: ActionBar strips newlines, Title splits at ||
let title_rendered = renderer.render(
    &catalog, 
    keys::WELCOME_TITLE, 
    OutputTarget::Title,  // auto-splits "Title||Subtitle"
    ctx,
)?;
```

**Types:**
- `Renderer` — built pipeline, takes target + context → `RenderedMessage`
- `OutputTarget` — `Chat`, `ActionBar`, `Title`, `BossBar`, `TabList`, `Kick`, `Console`
- `RenderMiddleware` — trait: `fn transform(&self, text: &str, target: OutputTarget) -> String`
- `RenderedMessage` — `{ text: String, target: OutputTarget, components: Option<Component> }`

---

### 6. Typed Placeholder Values

```rust
// Current: &[(&str, &str)] — all strings
// New: typed, lazy, formattable
let ctx = RenderContext::new(spacetime_id)
    .set("target", "Alice")
    .set("count", 42u32)                     // auto-format as "42"
    .set("duration", Duration::from_secs(300)) // auto-format as "5 minutes"
    .set("items", vec!["Sword", "Shield"])    // for {items|join:, }
    .lazy("heavy_stat", || expensive_lookup()); // only computed if used

// Value::Duration auto-formats: "5 minutes 30 seconds" / "2 hours" / "permanent"
```

**Types:**
- `RenderContext` — replaces `&[(&str, &str)]`
- `PlaceholderValue` — `Text(String)`, `Int(i64)`, `Float(f64)`, `Duration(Duration)`, `List(Vec<String>)`, `Lazy(Box<dyn Fn() -> String>)`

---

### 7. Message Variants

```rust
// Multiple variants, one selected at render time
MessageKey::new("motd.greeting")
    .variant("&aWelcome back, {username}!")
    .variant("&eGood to see you, {username}!")
    .variant("&bHey {username}, glad you're here!")
    .selection(VariantSelection::Random)    // or Weighted, RoundRobin, TimeBased, TierBased

// Time-based
.selection(VariantSelection::TimeBased {
    morning:   "&aGood morning, {username}!",
    afternoon: "&eGood afternoon, {username}!",
    evening:   "&9Good evening, {username}!",
})

// Rank-tier based
.selection(VariantSelection::TierBased(vec![
    (Tier::Vip, "&6VIP message"),
    (Tier::Default, "&7Regular message"),
]))
```

---

### 8. Message Chaining & Composition

```rust
// Concatenate message keys
let full_header = MsgChain::of(keys::STAFF_PREFIX)
    .then(keys::STAFF_ONLINE_HEADER)
    .separator("\n");

// Multi-part notification sequence
let sequence = MsgSequence::new()
    .after(Duration::ZERO,   keys::WELCOME_TITLE)
    .after(Duration::from_secs(2), keys::WELCOME_SUBTITLE)
    .after(Duration::from_secs(5), keys::SERVER_CONNECTING);
```

---

### 9. Observability & Dev Tools

```rust
// At startup: lint all registered templates
let report = Catalog::lint_all();
// Warns on: unknown {placeholder} tokens, empty defaults, duplicate keys,
// keys in CATALOG but never render()'d (dead keys), invalid & codes

// Runtime: track usage
Catalog::enable_tracking();
let unused = Catalog::unused_keys(); // keys defined but render() never called

// Admin audit log
Catalog::on_override(|key, old, new| {
    info!("message_config updated: {key} | {:?} -> {:?}", old, new);
});

// Introspect: list all keys matching a namespace
let friend_keys = Catalog::list_namespace("friend");
let overridden = Catalog::list_overridden(); // has admin message_config entry
```

---

## Implementation Plan

### Phase 1 — MVP (what to implement now)

1. **New standalone repo `Jingle`** containing a single `jingle` crate at its root (no workspace nesting needed unless the user adds one later)

2. **Core types** (`src/lib.rs`, `src/key.rs`):
   - `MessageKey` — replaces `MessageDefinition`, adds variant + metadata support
   - `Catalog` — replaces the static `CATALOG` slice, adds namespace indexing
   - `EntryBlock` — enhanced batch registration with priority + namespace
   - `RenderContext` — typed replacement for `&[(&str, &str)]`

3. **Template engine** (`src/template/`):
   - `Template::parse(s)` — parse `{key|modifier}`, `{if:cond}...{/if}`, `{style:x}...`
   - `Template::render(ctx)` — evaluate against a `RenderContext`
   - Supported: conditionals, plurals, join, default, nested key refs

4. **Style system** (`src/style.rs`):
   - `StyleRegistry` with `register(name, Style)`
   - `Style` builder with `.color()`, `.bold()`, gradient
   - Legacy `&` codes parser (ported from `internode-api/src/types/component.rs`)
   - Hex `{#RRGGBB}` and gradient `{gradient:a:b}` support

5. **Entry Block System** (`src/entry_block.rs`):
   - `EntryBlock::named(id).priority(p).namespace(ns).entry(k,v).conditional(c)`
   - `EntryBlock::merge(blocks)` — conflict resolution by priority
   - Serialize to `Vec<MessageDefaultEntry>` for SpacetimeDB compat

6. **Key namespace macro** (`src/macros.rs`):
   - `msg_keys! { pub mod staff { PREFIX = "staff.prefix"; } }`
   - Generates `pub const PREFIX: &'static str = "staff.prefix";` in a module

7. **Render pipeline** (`src/render.rs`):
   - `Renderer` with middleware chain
   - `OutputTarget` enum with target-aware transformations (Title splits at `||`)
   - Built-in middleware: `StripLegacyCodes`, `ConsoleStrip`, `GlobalSuffix`

8. **Dev tools** (`src/dev.rs`, behind `#[cfg(feature = "dev")]`):
   - `Catalog::lint_all()` — startup report
   - `Catalog::enable_tracking()` / `unused_keys()`
   - `Catalog::on_override(cb)`

### Phase 2 — Showcase polish

- Variant system (`VariantSelection::Random`, `TimeBased`, `TierBased`)
- Message sequences (`MsgSequence`)
- Message chaining (`MsgChain`)
- Typed `PlaceholderValue::Duration` with human formatting
- Lazy placeholder evaluation

---

## Internode Integration

Replace in `internode-core/src/commands/messages.rs`:
- `pub type Msg = MessageDefinition;` → `use keystone::MessageKey as Msg;`
- `pub static CATALOG: &[Msg]` → `keystone::Catalog` instance
- `render(cache, key, attrs)` → `renderer.render(&catalog, key, OutputTarget::Chat, ctx)`
- `register_catalog()` → `catalog.build_entry_block("internode-core").sync_to(spacetime)`

The `Cache::lang(key)` method stays as-is; `keystone` calls it via a `MessageStore` trait:

```rust
pub trait MessageStore: Send + Sync {
    fn get_override(&self, key: &str) -> Option<String>;
    fn get_default(&self, key: &str) -> Option<String>;
}
// Cache implements MessageStore
```

---

## Critical Files to Modify / Create

### In the new `Jingle` repo (all new)

| File | Purpose |
|------|---------|
| `Cargo.toml` | Crate `jingle`, edition 2021 |
| `src/lib.rs` | Public re-exports |
| `src/key.rs` | `MessageKey` |
| `src/catalog.rs` | `Catalog`, global singleton |
| `src/template/` | Parser + renderer |
| `src/placeholder.rs` | `RenderContext`, `PlaceholderValue` |
| `src/style.rs` | `Style`, `StyleRegistry` |
| `src/entry_block.rs` | `EntryBlock`, `Priority`, `Condition` |
| `src/render.rs` | `Renderer`, `OutputTarget`, middleware |
| `src/variants.rs` | `VariantSelection` |
| `src/compose.rs` | `MsgChain`, `MsgSequence` |
| `src/dev.rs` | `TemplateLinter`, `KeyTracker` (feature-gated) |
| `src/macros.rs` | `msg_keys!` macro |

### In Internode (after Jingle is ready)

| File | Change |
|------|--------|
| `Cargo.toml` (workspace) | Add `jingle = { path = "../Jingle" }` |
| `crates/internode-core/src/commands/messages.rs` | Migrate to `jingle` API |
| `crates/internode-api/src/types/component.rs` | Integrate with `jingle::Style` |
| `crates/internode-spacetime/src/connection.rs` | Implement `MessageStore` for `Cache` |

---

## Verification

1. `cargo build -p keystone` — crate compiles standalone
2. `cargo test -p keystone` — unit tests pass (template parsing, entry block merge, style registry)
3. `cargo build` (full workspace) — Internode still compiles with keystone integration
4. `cargo test -p internode-core` — existing `catalog_keys_are_unique` test still passes
5. Demo binary or integration test shows: parse template → render with context → verify output string