# Jingle — message formatting styles

Six ways to present the same examples. The two reference examples used throughout:

- **A** — single-line with placeholders and a conditional
- **B** — multi-line ban screen with duration, plural, and conditional

---

## Current style

Three separate fenced blocks per example: definition, context, rendered output.

---

### A — friend request sent

```
key:     "friend.add.request_sent"
default: "&6[Friend] &fRequest sent to {target}{if:mutual} &a(mutual){/if}."
```
```rust
RenderContext::new()
    .set("target", "Alice")
    .set("mutual", "true")
```
```
[Friend] Request sent to Alice (mutual).
```

---

### B — ban screen

```
key:     "ban.screen"
default: "&c&lYOU ARE BANNED\n&7Reason: &f{reason}\n&7Duration: &f{duration}\n&7Banned by: &f{staff|default:the system}"
```
```rust
RenderContext::new()
    .set("reason",   "Cheating")
    .set("duration", Duration::from_secs(7 * 24 * 3600))
    .set("staff",    "McMmax")
```
```
YOU ARE BANNED
Reason: Cheating
Duration: 7 days
Banned by: McMmax
```

---
---

## Style 1 — compact single block

Everything in one annotated code block. No separate fences.

---

### A
```
key      friend.add.request_sent
default  &6[Friend] &fRequest sent to {target}{if:mutual} &a(mutual){/if}.
ctx      target="Alice"  mutual="true"
out      [Friend] Request sent to Alice (mutual).
```

### B
```
key      ban.screen
default  &c&lYOU ARE BANNED\n&7Reason: &f{reason}\n&7Duration: &f{duration}\n&7Banned by: &f{staff|default:the system}
ctx      reason="Cheating"  duration=7d  staff="McMmax"
out      YOU ARE BANNED
         Reason: Cheating
         Duration: 7 days
         Banned by: McMmax
```

---
---

## Style 2 — YAML definition blocks

Each example is a single YAML block with `template`, `context`, and `renders` fields.

---

### A
```yaml
key: friend.add.request_sent
template: "&6[Friend] &fRequest sent to {target}{if:mutual} &a(mutual){/if}."
context:
  target: Alice
  mutual: "true"
renders: "[Friend] Request sent to Alice (mutual)."
```

### B
```yaml
key: ban.screen
template: |
  &c&lYOU ARE BANNED
  &7Reason: &f{reason}
  &7Duration: &f{duration}
  &7Banned by: &f{staff|default:the system}
context:
  reason: Cheating
  duration: 7 days
  staff: McMmax
renders: |
  YOU ARE BANNED
  Reason: Cheating
  Duration: 7 days
  Banned by: McMmax
```

---
---

## Style 3 — inline prose with code spans

No fenced blocks at all. Template and output live inline with the description.

---

### A

Key `friend.add.request_sent` — template: `&6[Friend] &fRequest sent to {target}{if:mutual} &a(mutual){/if}.`

With `target=Alice` and `mutual` present, renders to:
`[Friend] Request sent to Alice (mutual).`

Without `mutual`: `[Friend] Request sent to Alice.`

### B

Key `ban.screen` — multi-line template:
`&c&lYOU ARE BANNED\n&7Reason: &f{reason}\n&7Duration: &f{duration}\n&7Banned by: &f{staff|default:the system}`

With `reason=Cheating`, `duration=7d`, `staff=McMmax`:

`YOU ARE BANNED` / `Reason: Cheating` / `Duration: 7 days` / `Banned by: McMmax`

---
---

## Style 4 — arrow notation

Template and each placeholder on their own indented line with arrows. Output in a blockquote.

---

### A

**`friend.add.request_sent`**
```
  template  →  &6[Friend] &fRequest sent to {target}{if:mutual} &a(mutual){/if}.
  target    →  "Alice"
  mutual    →  "true"
```
> `[Friend] Request sent to Alice (mutual).`

### B

**`ban.screen`**
```
  template  →  &c&lYOU ARE BANNED\n&7Reason: &f{reason}\n...
  reason    →  "Cheating"
  duration  →  7 days
  staff     →  "McMmax"
```
> `YOU ARE BANNED`
> `Reason: Cheating`
> `Duration: 7 days`
> `Banned by: McMmax`

---
---

## Style 5 — table per example

Each example is a two-column table: left column labels, right column values.

---

### A

| | |
|---|---|
| **key** | `friend.add.request_sent` |
| **template** | `&6[Friend] &fRequest sent to {target}{if:mutual} &a(mutual){/if}.` |
| **target** | `"Alice"` |
| **mutual** | `"true"` |
| **output** | `[Friend] Request sent to Alice (mutual).` |

### B

| | |
|---|---|
| **key** | `ban.screen` |
| **template** | `&c&lYOU ARE BANNED\n&7Reason: &f{reason}\n&7Duration: &f{duration}\n&7Banned by: &f{staff\|default:the system}` |
| **reason** | `"Cheating"` |
| **duration** | `7 days` |
| **staff** | `"McMmax"` |
| **output** | `YOU ARE BANNED` / `Reason: Cheating` / `Duration: 7 days` / `Banned by: McMmax` |

---
---

## Full example set (current style)

The rest of this file uses the current style.

---

### Template syntax

| syntax | what it does |
|---|---|
| `{player}` | substitute placeholder value |
| `{player\|default:Someone}` | fallback when key absent |
| `{player\|fallback:Someone}` | fallback when key empty or absent |
| `{count\|plural::s}` | appends `s` when count ≠ 1 |
| `{count\|plural:person:people}` | full singular / plural forms |
| `{names\|join:, }` | join a list with a separator |
| `{if:in_party}...{/if}` | show block only when key is present |
| `{if:!in_party}...{/if}` | show block only when key is absent |
| `{key:staff.prefix}` | embed the resolved text of another key |
| `{style:error}...{/style}` | apply a named style from the StyleRegistry |
| `{gradient:#FF0000:#0000FF}...{/gradient}` | MiniMessage gradient |
| `{#FF5500}...{/}` | inline hex color |
| `&c`, `&l`, `&r` ... | legacy Minecraft color / format codes |

Newlines in a default string (`\n`) produce real line breaks in chat.

---

## Single-line messages

### Plain substitution
```
key:     "chat.format"
default: "&7[&f{player}&7] &f{message}"
```
```rust
RenderContext::new()
    .set("player", "McMmax")
    .set("message", "hello everyone")
```
```
[McMmax] hello everyone
```

---

### Default when absent
```
key:     "join.greeting"
default: "Welcome{display_name|default:}, {player}!"
```
```rust
RenderContext::new().set("player", "McMmax")
// → Welcome, McMmax!

RenderContext::new().set("player", "McMmax").set("display_name", " back")
// → Welcome back, McMmax!
```

---

### Plural form
```
key:     "friend.count"
default: "You have {count} friend{count|plural::s}."
```
```rust
RenderContext::new().set("count", 1i64)  // → You have 1 friend.
RenderContext::new().set("count", 4i64)  // → You have 4 friends.
```

---

### List join
```
key:     "party.members"
default: "Party: {members|join:, }"
```
```rust
RenderContext::new().set("members", vec!["Alice", "Bob", "Carol"])
```
```
Party: Alice, Bob, Carol
```

---

### Duration
```
key:     "ban.notice"
default: "&cYou are banned for {duration}."
```
```rust
RenderContext::new().set("duration", Duration::from_secs(3 * 3600))
// → You are banned for 3 hours.

RenderContext::new().set("duration", Duration::from_secs(30 * 24 * 3600))
// → You are banned for permanent.
```

---

### Conditional block
```
key:     "queue.status"
default: "Position {pos} in queue{if:eta} — est. {eta}{/if}."
```
```rust
RenderContext::new().set("pos", 4i64)
// → Position 4 in queue.

RenderContext::new().set("pos", 4i64).set("eta", "2 minutes")
// → Position 4 in queue — est. 2 minutes.
```

---

### Negated conditional
```
key:     "party.invite"
default: "{player} invited you to a party.{if:!in_party} Type /accept to join.{/if}"
```
```rust
RenderContext::new().set("player", "Alice")
// → Alice invited you to a party. Type /accept to join.

RenderContext::new().set("player", "Alice").set("in_party", "true")
// → Alice invited you to a party.
```

---

### Key reference
```
key:     "staff.prefix"
default: "&4[Staff] "

key:     "staff.freeze.frozen"
default: "{key:staff.prefix}&bFroze {target}."
```
```rust
RenderContext::new().set("target", "Bob")
```
```
[Staff] Froze Bob.
```

---

### Named style
```rust
reg.register("error",   Style::new().color("#FF5555").bold());
reg.register("success", Style::new().color("#55FF55"));
reg.register("warn",    Style::new().color("gold"));
```
```
key:     "cmd.error"
default: "{style:error}Unknown command.{/style} Type /help for a list."
```
```
<color:#FF5555><bold>Unknown command.</bold></color><reset> Type /help for a list.
```

---

### Hex color
```
key:     "rank.display"
default: "{#FF8800}{rank}{/} {player}"
```
```rust
RenderContext::new().set("rank", "[MVP]").set("player", "McMmax")
```
```
<#FF8800>[MVP]</#FF8800> McMmax
```

---

### Gradient
```
key:     "motd.title"
default: "{gradient:#FF0000:#AA00FF}WELCOME TO THE SERVER{/gradient}"
```
```
<gradient:#FF0000:#AA00FF>WELCOME TO THE SERVER</gradient>
```

---

## Multi-line messages

### Welcome MOTD
```
key:     "motd.join"
default: "&8&m                    &r\n&6Welcome, &e{player}&6!\n&8&m                    &r"
```
```rust
RenderContext::new().set("player", "McMmax")
```
```
────────────────────
Welcome, McMmax!
────────────────────
```

---

### Ban screen
```
key:     "ban.screen"
default: "&c&lYOU ARE BANNED\n&7Reason: &f{reason}\n&7Duration: &f{duration}\n&7Appeal at: &bdiscord.gg/example\n&7Banned by: &f{staff}"
```
```rust
RenderContext::new()
    .set("reason",   "Cheating")
    .set("duration", Duration::from_secs(7 * 24 * 3600))
    .set("staff",    "McMmax")
```
```
YOU ARE BANNED
Reason: Cheating
Duration: 7 days
Appeal at: discord.gg/example
Banned by: McMmax
```

---

### Staff alert with conditional detail line
```
key:     "staff.alert"
default: "&4[ALERT] &f{player} flagged for {reason}.{if:details}\n&7Details: &f{details}{/if}"
```
```rust
RenderContext::new().set("player", "Bob").set("reason", "reach")
```
```
[ALERT] Bob flagged for reach.
```
```rust
RenderContext::new()
    .set("player",  "Bob")
    .set("reason",  "reach")
    .set("details", "avg 4.8 blocks, 12 samples")
```
```
[ALERT] Bob flagged for reach.
Details: avg 4.8 blocks, 12 samples
```

---

### Party invite with member list
```
key:     "party.invite_detail"
default: "&6{inviter} &einvites you to their party!\n&7Members: &f{members|join:, }\n&aType &f/party accept &ato join."
```
```rust
RenderContext::new()
    .set("inviter", "Alice")
    .set("members", vec!["Alice", "Bob", "Carol"])
```
```
Alice invites you to their party!
Members: Alice, Bob, Carol
Type /party accept to join.
```

---

### Achievement unlock
```
key:     "achievement.unlock"
default: "{gradient:#FFD700:#FF8C00}✦ Achievement Unlocked ✦{/gradient}\n&e{name}\n&7{description}"
```
```rust
RenderContext::new()
    .set("name",        "Speed Demon")
    .set("description", "Win a race in under 60 seconds.")
```
```
✦ Achievement Unlocked ✦    ← gold→orange gradient
Speed Demon
Win a race in under 60 seconds.
```

---

### Modlog summary
```
key:     "modlog.summary"
default: "&7History for &f{target}&7:\n&c{ban_count} ban{ban_count|plural::s}{if:mute_count}, &e{mute_count} mute{mute_count|plural::s}{/if}\n&7Last action: &f{last_action|default:none}"
```
```rust
RenderContext::new()
    .set("target",      "Bob")
    .set("ban_count",   2i64)
    .set("mute_count",  1i64)
    .set("last_action", "7-day ban")
```
```
History for Bob:
2 bans, 1 mute
Last action: 7-day ban
```
```rust
RenderContext::new()
    .set("target",    "Alice")
    .set("ban_count", 1i64)
```
```
History for Alice:
1 ban
Last action: none
```

---

## Variant messages

### Random MOTD
```rust
MessageKey::new("motd.random", "Have a great session!")
    .variant("Stay hydrated.")
    .variant("GGs only.")
    .variant("Report bugs at discord.gg/example.")
    .selection(VariantSelection::Random)
```

### Weighted tip-of-the-day
```rust
MessageKey::new("tip.daily", "Use /help to see all commands.")
    .selection(VariantSelection::Weighted(vec![
        (5, "Use /help to see all commands."),
        (3, "Join our Discord at discord.gg/example."),
        (1, "Enjoying the server? Leave a review!"),
    ]))
```

### Time-based greeting
```rust
MessageKey::new("greeting.time", "Good morning, {player}!")
    .selection(VariantSelection::TimeBased {
        morning:   "Good morning, {player}!",
        afternoon: "Good afternoon, {player}!",
        evening:   "Good evening, {player}!",
    })
```

### Tier-based rank message
```rust
MessageKey::new("rank.welcome", "Welcome, {player}.")
    .selection(VariantSelection::TierBased(vec![
        (Tier::Staff,   "&4[Staff] Welcome back, {player}."),
        (Tier::Elite,   "&5[Elite] Welcome, {player}!"),
        (Tier::VipPlus, "&6[VIP+] Welcome, {player}!"),
        (Tier::Vip,     "&e[VIP] Welcome, {player}!"),
    ]))
```
