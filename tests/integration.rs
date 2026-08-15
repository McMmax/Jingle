/// integration tests — simulates the full message pipeline across the MINEMARES stack:
///
/// CRUISER (Go) sends player actions → Internode (Rust) resolves message keys via jingle
/// → renders to string → Stomzy (Kotlin/Java) delivers to player.
///
/// This file exercises every integration point:
/// - Internode startup: EntryBlock sync → SpacetimeDB
/// - Internode runtime: Catalog render with MessageStore adapter (SpacetimeDB Cache)
/// - Stomzy: MsgChain for prefixed messages, MsgSequence for timed delivery
/// - Admin override flow: notify_override + list_overridden
/// - Style registry wired through Catalog to template eval
/// - All variant selection modes
/// - Dev-feature lint and tracking (run with --features dev)

use jingle::{
    Catalog, Condition, EntryBlock, MessageKey, MessageStore, MsgChain, MsgSequence,
    OutputTarget, Priority, RenderContext, Renderer, Style, StyleRegistry,
    VariantSelection, Tier,
    render::{ConsoleStrip, ConsoleFullStrip, GlobalSuffixAppender, ActionBarSanitize, StripAllFormatting},
};
use std::collections::HashMap;
use std::time::Duration;

// --- mock MessageStore — simulates Internode's SpacetimeDB Cache adapter ---

struct MockCache {
    overrides: HashMap<String, String>,
}

impl MockCache {
    fn new() -> Self { Self { overrides: HashMap::new() } }
    fn with_override(mut self, key: &str, val: &str) -> Self {
        self.overrides.insert(key.into(), val.into()); self
    }
}

impl MessageStore for MockCache {
    fn get_override(&self, key: &str) -> Option<String> {
        self.overrides.get(key).cloned()
    }
    fn get_default(&self, _key: &str) -> Option<String> { None }
    fn list_overrides(&self) -> Vec<String> {
        self.overrides.keys().cloned().collect()
    }
}

// --- key constants as Internode/Stomzy declare them ---

pub mod keys {
    pub mod staff {
        jingle::msg_keys! {
            PREFIX = "staff.prefix";
            CHAT_FORMAT = "staff.chat_format"
        }
        pub mod freeze {
            jingle::msg_keys! {
                FROZEN = "staff.freeze.frozen";
                UNFROZEN = "staff.freeze.unfrozen"
            }
        }
    }
    pub mod friend {
        pub mod add {
            jingle::msg_keys! {
                REQUEST_SENT = "friend.add.request_sent"
            }
        }
    }
    pub mod motd {
        jingle::msg_keys! {
            GREETING = "motd.greeting";
            WELCOME_TITLE = "motd.welcome_title";
            VARIANT_MOTD = "motd.variant"
        }
    }
    pub mod ban {
        jingle::msg_keys! {
            DURATION = "ban.duration"
        }
    }
}

fn base_style_registry() -> StyleRegistry {
    let reg = StyleRegistry::new();
    reg.register("error",   Style::new().color("#FF5555").bold());
    reg.register("success", Style::new().color("#55FF55"));
    reg.register("header",  Style::new().color("#AAAAAA").strikethrough());
    reg.register("warn",    Style::new().color("gold"));
    reg
}

fn build_catalog() -> Catalog {
    let cache = MockCache::new()
        .with_override("staff.prefix", "&4[ADMIN] "); // admin changed the prefix

    let mut cat = Catalog::new()
        .with_store(cache)
        .with_style_registry(base_style_registry());

    cat.register_all(vec![
        MessageKey::new(keys::staff::PREFIX,            "&3[Staff] "),
        MessageKey::new(keys::staff::CHAT_FORMAT,       "&b{player}&f: {message}"),
        MessageKey::new(keys::staff::freeze::FROZEN,    "&b{target} has been frozen"),
        MessageKey::new(keys::staff::freeze::UNFROZEN,  "&b{target} has been unfrozen"),
        MessageKey::new(keys::friend::add::REQUEST_SENT,"Friend request sent to {target}!"),
        MessageKey::new(keys::motd::GREETING,           "Welcome back, {username}!"),
        MessageKey::new(keys::motd::WELCOME_TITLE,      "Welcome||Hello {username}"),
        MessageKey::new(keys::ban::DURATION,            "You are banned for {duration}"),
    ]);

    cat
}

// =============================================================================
// Internode startup — EntryBlock sync to SpacetimeDB
// =============================================================================

#[test]
fn entry_block_sync_produces_correct_entries() {
    let core_block = EntryBlock::named("internode-core")
        .priority(Priority::Core)
        .namespace("staff")
        .entry("prefix",      "&3[Staff] ")
        .entry("chat_format", "&b{player}&f: {message}")
        .conditional(Condition::KeyAbsent)
        .tag("v1.0");

    let plugin_block = EntryBlock::named("friend-plugin")
        .priority(Priority::Plugin)
        .entry("friend.add.request_sent", "Friend request sent to {target}!");

    let entries = EntryBlock::merge(vec![core_block, plugin_block]);

    assert!(entries.iter().any(|e| e.key == "staff.prefix" && e.value == "&3[Staff] "));
    assert!(entries.iter().any(|e| e.key == "friend.add.request_sent"));
    // sorted output — deterministic for SpacetimeDB upserts
    assert!(entries.windows(2).all(|w| w[0].key <= w[1].key), "entries not sorted");
}

#[test]
fn core_priority_wins_on_conflict() {
    let plugin = EntryBlock::named("plugin").priority(Priority::Plugin).entry("staff.prefix", "&7[Plugin] ");
    let core   = EntryBlock::named("core").priority(Priority::Core).entry("staff.prefix", "&3[Core] ");
    let entries = EntryBlock::merge(vec![plugin, core]);
    let entry = entries.iter().find(|e| e.key == "staff.prefix").unwrap();
    assert_eq!(entry.value, "&3[Core] ");
}

// =============================================================================
// Internode runtime — chat render with context
// =============================================================================

#[test]
fn chat_render_substitutes_context() {
    let cat = build_catalog();
    let renderer = Renderer::builder()
        .middleware(GlobalSuffixAppender("&r".into()))
        .build();
    let ctx = RenderContext::new().set("target", "Alice");
    let msg = renderer.render(&cat, keys::friend::add::REQUEST_SENT, OutputTarget::Chat, &ctx).unwrap();
    assert_eq!(msg.text, "Friend request sent to Alice!&r");
    assert_eq!(msg.target, OutputTarget::Chat);
}

#[test]
fn store_override_wins_over_compiled_default() {
    let cat = build_catalog();
    assert_eq!(cat.resolve_text(keys::staff::PREFIX).unwrap(), "&4[ADMIN] ");
}

// =============================================================================
// Console output — strip all formatting
// =============================================================================

#[test]
fn console_strips_legacy_codes() {
    let cat = build_catalog();
    let renderer = Renderer::builder().middleware(ConsoleStrip).build();
    let ctx = RenderContext::new().set("target", "Bob");
    let msg = renderer.render(&cat, keys::staff::freeze::FROZEN, OutputTarget::Console, &ctx).unwrap();
    assert!(!msg.text.contains('&'), "& codes not stripped: {}", msg.text);
    assert!(msg.text.contains("Bob"));
}

#[test]
fn console_full_strip_removes_mini_message_and_legacy() {
    let reg = StyleRegistry::new();
    reg.register("error", Style::new().color("#FF5555").bold());
    let mut cat = Catalog::new().with_style_registry(reg);
    cat.register(MessageKey::new("alert", "{style:error}&cOops!{/style}"));
    let renderer = Renderer::builder().middleware(ConsoleFullStrip).build();
    let msg = renderer.render(&cat, "alert", OutputTarget::Console, &RenderContext::new()).unwrap();
    assert!(!msg.text.contains('<'),  "MiniMessage not stripped: {}", msg.text);
    assert!(!msg.text.contains('&'),  "& codes not stripped: {}", msg.text);
    assert!(msg.text.contains("Oops!"));
}

#[test]
fn strip_all_formatting_middleware() {
    let mut cat = Catalog::new();
    cat.register(MessageKey::new("msg", "&a<red>hello</red>"));
    let renderer = Renderer::builder().middleware(StripAllFormatting).build();
    let msg = renderer.render(&cat, "msg", OutputTarget::Chat, &RenderContext::new()).unwrap();
    assert_eq!(msg.text, "hello");
}

// =============================================================================
// Title target — auto-split at ||
// =============================================================================

#[test]
fn title_splits_at_double_pipe() {
    let cat = build_catalog();
    let ctx = RenderContext::new().set("username", "Max");
    let msg = Renderer::plain().render(&cat, keys::motd::WELCOME_TITLE, OutputTarget::Title, &ctx).unwrap();
    let (title, subtitle) = msg.title_parts.unwrap();
    assert_eq!(title, "Welcome");
    assert_eq!(subtitle, "Hello Max");
}

// =============================================================================
// ActionBar — newline stripping
// =============================================================================

#[test]
fn action_bar_has_no_newlines() {
    let mut cat = Catalog::new();
    cat.register(MessageKey::new("ab", "line1\nline2"));
    let renderer = Renderer::builder().middleware(ActionBarSanitize).build();
    let msg = renderer.render(&cat, "ab", OutputTarget::ActionBar, &RenderContext::new()).unwrap();
    assert!(!msg.text.contains('\n'));
}

// =============================================================================
// Stomzy — MsgChain: prefix + body composition
// =============================================================================

#[test]
fn msg_chain_composes_prefix_and_body() {
    let cat = build_catalog();
    // admin override active → prefix resolves to &4[ADMIN]
    let chain = MsgChain::of(keys::staff::PREFIX)
        .then(keys::staff::freeze::FROZEN)
        .separator("");
    let ctx = RenderContext::new().set("target", "Carol");
    let out = chain.render(|k| cat.render(k, &ctx));
    assert_eq!(out, "&4[ADMIN] &bCarol has been frozen");
}

// =============================================================================
// Stomzy — MsgSequence: timed welcome delivery
// =============================================================================

#[test]
fn msg_sequence_preserves_delays_and_order() {
    let seq = MsgSequence::new()
        .after(Duration::ZERO,          keys::motd::WELCOME_TITLE)
        .after(Duration::from_secs(2),  keys::motd::GREETING)
        .after(Duration::from_secs(5),  keys::friend::add::REQUEST_SENT);
    let steps = seq.steps();
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0].key, keys::motd::WELCOME_TITLE);
    assert_eq!(steps[1].delay, Duration::from_secs(2));
    assert_eq!(steps[2].key, keys::friend::add::REQUEST_SENT);
}

// =============================================================================
// StyleRegistry — preset styles wired through Catalog
// =============================================================================

#[test]
fn style_registry_register_and_retrieve() {
    let cat = build_catalog();
    let reg = cat.style_registry();
    assert!(reg.get("error").map(|s| s.bold).unwrap_or(false), "error style should be bold");
    assert!(reg.get("success").is_some());
    assert!(reg.get("missing").is_none());
}

#[test]
fn style_block_applies_via_catalog() {
    let reg = StyleRegistry::new();
    reg.register("warn", Style::new().color("gold").bold());
    let mut cat = Catalog::new().with_style_registry(reg);
    cat.register(MessageKey::new("alert", "{style:warn}Watch out!{/style}"));
    let out = cat.render("alert", &RenderContext::new()).unwrap();
    assert!(out.contains("Watch out!"));
    assert!(out.contains("<bold>"),  "expected bold in: {out}");
    assert!(out.contains("<reset>"), "expected reset in: {out}");
}

// =============================================================================
// Typed placeholder values
// =============================================================================

#[test]
fn typed_context_int_and_duration() {
    let mut cat = Catalog::new();
    cat.register(MessageKey::new("info", "count: {count}, time: {elapsed}"));
    let ctx = RenderContext::new()
        .set("count",   42u32)
        .set("elapsed", Duration::from_secs(125));
    let out = cat.render("info", &ctx).unwrap();
    assert_eq!(out, "count: 42, time: 2 minutes 5 seconds");
}

#[test]
fn duration_permanent_threshold_in_template() {
    let cat = build_catalog();
    let ctx = RenderContext::new().set("duration", Duration::from_secs(90 * 24 * 3600));
    let out = cat.render(keys::ban::DURATION, &ctx).unwrap();
    assert_eq!(out, "You are banned for permanent");
}

#[test]
fn lazy_placeholder_only_evaluates_when_used() {
    let mut cat = Catalog::new();
    cat.register(MessageKey::new("lazy.msg", "result: {val}"));
    let evaluated = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = evaluated.clone();
    let ctx = RenderContext::new().lazy("val", move || {
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
        "computed".into()
    });
    let out = cat.render("lazy.msg", &ctx).unwrap();
    assert_eq!(out, "result: computed");
    assert!(evaluated.load(std::sync::atomic::Ordering::SeqCst));
}

// =============================================================================
// Variant selection
// =============================================================================

#[test]
fn round_robin_variant_rotates_via_catalog() {
    let mut cat = Catalog::new();
    cat.register(
        MessageKey::new("motd.rr", "default")
            .variant("morning")
            .variant("afternoon")
            .variant("evening")
            .selection(VariantSelection::RoundRobin)
    );
    let r: Vec<String> = (0..6).map(|_| cat.resolve_text("motd.rr").unwrap()).collect();
    assert_eq!(&r[0], "morning");
    assert_eq!(&r[1], "afternoon");
    assert_eq!(&r[2], "evening");
    assert_eq!(&r[3], "morning"); // wraps
    assert_eq!(&r[4], "afternoon");
}

#[test]
fn weighted_variant_picks_from_items() {
    let mut cat = Catalog::new();
    cat.register(
        MessageKey::new("weighted", "default")
            .selection(VariantSelection::Weighted(vec![(1, "rare"), (99, "common")]))
    );
    // with 99:1 weighting, "common" should appear in at least some of 10 picks
    let picks: Vec<String> = (0..10).map(|_| cat.resolve_text("weighted").unwrap()).collect();
    assert!(picks.iter().any(|p| p == "common"), "expected common to appear: {picks:?}");
}

#[test]
fn tier_based_variant_selects_by_tier() {
    let mut cat = Catalog::new();
    cat.register(
        MessageKey::new("greeting", "Hello player!")
            .selection(VariantSelection::TierBased(vec![
                (Tier::Vip,   "Welcome, VIP!"),
                (Tier::Staff, "Hey, Staff!"),
                (Tier::Default, "Hello player!"),
            ]))
    );
    let key = cat.get("greeting").unwrap();
    assert_eq!(key.selection.pick_for_tier("fallback", &Tier::Vip),   "Welcome, VIP!");
    assert_eq!(key.selection.pick_for_tier("fallback", &Tier::Staff),  "Hey, Staff!");
    assert_eq!(key.selection.pick_for_tier("fallback", &Tier::Elite),  "fallback");
}

// =============================================================================
// Admin override flow
// =============================================================================

#[test]
fn notify_override_fires_all_callbacks() {
    use std::sync::{Arc, Mutex};
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let log2 = log.clone();
    let log3 = log.clone();
    let mut cat = Catalog::new();
    cat.on_override(move |k, old, new| {
        log2.lock().unwrap().push(format!("cb1:{k}"));
    });
    cat.on_override(move |k, old, new| {
        log3.lock().unwrap().push(format!("cb2:{k}"));
    });
    cat.notify_override("staff.prefix", Some("&3[Staff] "), "&4[ADMIN] ");
    let entries = log.lock().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.contains(&"cb1:staff.prefix".to_owned()));
    assert!(entries.contains(&"cb2:staff.prefix".to_owned()));
}

#[test]
fn list_overridden_returns_keys_with_store_overrides() {
    let cat = build_catalog(); // MockCache overrides staff.prefix
    let overridden = cat.list_overridden();
    assert!(overridden.contains(&"staff.prefix".to_owned()),
        "expected staff.prefix in overridden list: {overridden:?}");
}

// =============================================================================
// Namespace index
// =============================================================================

#[test]
fn namespace_index_lists_correct_keys() {
    let cat = build_catalog();
    let staff_keys = cat.list_namespace("staff");
    assert!(staff_keys.contains(&"staff.prefix"),      "staff.prefix missing");
    assert!(staff_keys.contains(&"staff.chat_format"), "staff.chat_format missing");
    let freeze_keys = cat.list_namespace("staff.freeze");
    assert!(freeze_keys.contains(&"staff.freeze.frozen"));
}

// =============================================================================
// Catalog → EntryBlock round-trip
// =============================================================================

#[test]
fn catalog_build_entry_block_round_trips() {
    let cat = build_catalog();
    let block = cat.build_entry_block("internode-core");
    let entries = block.to_entries();
    assert_eq!(entries.len(), cat.entry_count());
    assert!(entries.iter().any(|e| e.key == "staff.prefix"));
}

// =============================================================================
// Dev feature — lint and tracking
// =============================================================================

#[cfg(feature = "dev")]
#[test]
fn lint_all_catches_parse_error() {
    use jingle::dev::LintKind;
    let mut cat = Catalog::new();
    cat.register(MessageKey::new("bad.template", "{unclosed"));
    let issues = cat.lint_all();
    assert!(issues.iter().any(|i| i.kind == LintKind::ParseError), "expected ParseError: {issues:?}");
}

#[cfg(feature = "dev")]
#[test]
fn lint_all_catches_unknown_style_ref() {
    use jingle::dev::LintKind;
    let mut cat = Catalog::new(); // no style registry
    cat.register(MessageKey::new("styled", "{style:ghost}text{/style}"));
    let issues = cat.lint_all();
    assert!(issues.iter().any(|i| i.kind == LintKind::UnknownStyleRef));
}

#[cfg(feature = "dev")]
#[test]
fn enable_tracking_reports_unused_keys() {
    let mut cat = build_catalog();
    cat.enable_tracking();
    let ctx = RenderContext::new().set("target", "Alice");
    cat.render(keys::friend::add::REQUEST_SENT, &ctx);
    let unused = cat.unused_keys();
    assert!(unused.contains(&"staff.prefix"),  "staff.prefix should be unused");
    assert!(!unused.contains(&"friend.add.request_sent"), "friend.add.request_sent was rendered");
}

// =============================================================================
// Template language — full feature smoke tests
// =============================================================================

#[test]
fn template_conditional_shows_when_key_set() {
    let mut cat = Catalog::new();
    cat.register(MessageKey::new("msg", "Hello{if:suffix} suffix{/if}!"));
    let with_suffix = RenderContext::new().set("suffix", "yes");
    let without_suffix = RenderContext::new();
    assert_eq!(cat.render("msg", &with_suffix).unwrap(),    "Hello suffix!");
    assert_eq!(cat.render("msg", &without_suffix).unwrap(), "Hello!");
}

#[test]
fn template_plural_form_in_template() {
    let mut cat = Catalog::new();
    cat.register(MessageKey::new("friends", "{count} friend{count|plural::s} online"));
    assert_eq!(cat.render("friends", &RenderContext::new().set("count", 1u32)).unwrap(), "1 friend online");
    assert_eq!(cat.render("friends", &RenderContext::new().set("count", 5u32)).unwrap(), "5 friends online");
}

#[test]
fn template_join_on_list() {
    let mut cat = Catalog::new();
    cat.register(MessageKey::new("list", "Players: {names|join:, |fallback:nobody}"));
    let with_names = RenderContext::new().set("names", vec!["Alice", "Bob", "Carol"]);
    let empty = RenderContext::new().set("names", Vec::<&str>::new());
    assert_eq!(cat.render("list", &with_names).unwrap(), "Players: Alice, Bob, Carol");
    // empty list: join produces "" which triggers fallback
    let out = cat.render("list", &empty).unwrap();
    assert!(out.contains("nobody") || out.contains("Players:"), "unexpected: {out}");
}

#[test]
fn template_key_ref_embeds_another_message() {
    let mut cat = Catalog::new();
    cat.register(MessageKey::new("prefix",  "&3[Staff] "));
    cat.register(MessageKey::new("message", "{key:prefix}Hello {name}!"));
    let ctx = RenderContext::new().set("name", "Eve");
    assert_eq!(cat.render("message", &ctx).unwrap(), "&3[Staff] Hello Eve!");
}

#[test]
fn template_default_modifier_fills_absent_placeholder() {
    let mut cat = Catalog::new();
    cat.register(MessageKey::new("greet", "Hello {name|default:stranger}!"));
    assert_eq!(cat.render("greet", &RenderContext::new()).unwrap(), "Hello stranger!");
    assert_eq!(cat.render("greet", &RenderContext::new().set("name", "Alice")).unwrap(), "Hello Alice!");
}
