/// how a message key selects among multiple variant strings
#[derive(Debug, Clone, Default)]
pub enum VariantSelection {
    #[default]
    Fixed,
    Random,
    /// stateless fallback always picks index 0 — Catalog tracks the real counter per key
    RoundRobin,
    /// weighted random: each entry is (weight, text); higher weight = more likely
    Weighted(Vec<(u32, &'static str)>),
    TimeBased {
        morning: &'static str,
        afternoon: &'static str,
        evening: &'static str,
    },
    TierBased(Vec<(Tier, &'static str)>),
}

/// player rank tiers — used by TierBased variant selection
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Default,
    Vip,
    VipPlus,
    Elite,
    Staff,
}

impl VariantSelection {
    /// picks a variant text — RoundRobin falls back to index 0; use Catalog for true rotation.
    /// Weighted uses its own internal items list; all other variants use the `variants` slice.
    pub fn pick<'a>(&self, default: &'a str, variants: &[&'a str]) -> &'a str {
        match self {
            // Weighted owns its item list — variants slice is irrelevant
            Self::Weighted(items) => pick_weighted(default, items),
            // TimeBased and TierBased don't use the variants slice either
            Self::TimeBased { morning, afternoon, evening } => {
                pick_time_based(morning, afternoon, evening)
            }
            Self::TierBased(_) => default, // tier resolved by pick_for_tier
            Self::Fixed => default,
            _ => {
                if variants.is_empty() { return default; }
                match self {
                    // PERF(max): lcg avoids pulling rand dep for a single pick
                    Self::Random => variants[lcg_pick(variants.len())],
                    Self::RoundRobin => variants[0], // Catalog handles real rotation
                    _ => default,
                }
            }
        }
    }

    /// picks a variant based on counter — used by Catalog for true round-robin rotation
    pub fn pick_round_robin<'a>(&self, default: &'a str, variants: &[&'a str], counter: &mut usize) -> &'a str {
        if variants.is_empty() { return default; }
        let idx = *counter % variants.len();
        *counter = counter.wrapping_add(1);
        variants[idx]
    }

    pub fn pick_for_tier<'a>(&self, default: &'a str, tier: &Tier) -> &'a str {
        if let Self::TierBased(pairs) = self {
            for (t, text) in pairs {
                if t == tier {
                    return text;
                }
            }
        }
        default
    }
}

fn pick_weighted<'a>(default: &'a str, items: &[(u32, &'a str)]) -> &'a str {
    if items.is_empty() { return default; }
    let total: u32 = items.iter().map(|(w, _)| w).sum();
    if total == 0 { return default; }
    let roll = lcg_pick(total as usize) as u32;
    let mut acc = 0u32;
    for (weight, text) in items {
        acc += weight;
        if roll < acc { return text; }
    }
    items.last().map(|(_, t)| *t).unwrap_or(default)
}

fn lcg_pick(len: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    // PERF(max): xorshift64 on nanos — stateless, avoids rand dep, good distribution
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(42);
    let mut x = seed ^ (seed << 13);
    x ^= x >> 7;
    x ^= x << 17;
    (x as usize) % len
}

fn pick_time_based<'a>(morning: &'a str, afternoon: &'a str, evening: &'a str) -> &'a str {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // hour in UTC — callers in a server context may offset for local tz
    let hour = (secs / 3600) % 24;
    match hour {
        5..=11 => morning,
        12..=17 => afternoon,
        _ => evening,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_returns_default() {
        let sel = VariantSelection::Fixed;
        assert_eq!(sel.pick("hello", &["a", "b"]), "hello");
    }

    #[test]
    fn random_returns_some_variant() {
        let variants = &["a", "b", "c"];
        let sel = VariantSelection::Random;
        let result = sel.pick("default", variants);
        assert!(variants.contains(&result), "result {result:?} not in variants");
    }

    #[test]
    fn round_robin_rotates_via_counter() {
        let sel = VariantSelection::RoundRobin;
        let variants = &["x", "y", "z"];
        let mut counter = 0;
        assert_eq!(sel.pick_round_robin("d", variants, &mut counter), "x");
        assert_eq!(sel.pick_round_robin("d", variants, &mut counter), "y");
        assert_eq!(sel.pick_round_robin("d", variants, &mut counter), "z");
        assert_eq!(sel.pick_round_robin("d", variants, &mut counter), "x"); // wraps
    }

    #[test]
    fn weighted_respects_zero_weight() {
        // zero-weight entry never picked; one-weight always picked
        let sel = VariantSelection::Weighted(vec![(0, "never"), (1, "always")]);
        for _ in 0..20 {
            assert_eq!(sel.pick("d", &["irrelevant"]), "always");
        }
    }

    #[test]
    fn weighted_all_zero_returns_default() {
        let sel = VariantSelection::Weighted(vec![(0, "a"), (0, "b")]);
        assert_eq!(sel.pick("fallback", &[]), "fallback");
    }

    #[test]
    fn weighted_empty_items_returns_default() {
        let sel = VariantSelection::Weighted(vec![]);
        assert_eq!(sel.pick("only", &["irrelevant"]), "only");
    }

    #[test]
    fn weighted_single_entry_always_picks_it() {
        let sel = VariantSelection::Weighted(vec![(5, "only")]);
        for _ in 0..10 {
            assert_eq!(sel.pick("d", &[]), "only");
        }
    }

    #[test]
    fn tier_based_matches_tier() {
        let sel = VariantSelection::TierBased(vec![
            (Tier::Vip, "vip msg"),
            (Tier::Default, "default msg"),
        ]);
        assert_eq!(sel.pick_for_tier("fallback", &Tier::Vip), "vip msg");
        assert_eq!(sel.pick_for_tier("fallback", &Tier::Elite), "fallback");
    }

    #[test]
    fn empty_variants_returns_default_for_non_weighted() {
        // Random, Fixed, RoundRobin all use the variants slice — empty → default
        assert_eq!(VariantSelection::Random.pick("only", &[]), "only");
        assert_eq!(VariantSelection::Fixed.pick("only", &[]), "only");
        assert_eq!(VariantSelection::RoundRobin.pick("only", &[]), "only");
    }
}
