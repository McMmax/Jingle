# MINEMARES Code Style Guide

Stack: **CRUISER** (Go) → **Internode** (Rust) → **Stomzy** (Java/Kotlin + Rust SDK via BoltFFI/Panama) → **SpacetimeDB**

---

## Philosophy

Latency is the law. Every decision flows from this priority order:

1. **Latency** — combat path is king, nothing blocks it
2. **Bandwidth** — minimize bytes on the wire, always
3. **Scaling** — must hold under 20v20, not just 1v1
4. **Readability** — clean, never at the cost of 1–3
5. **Brevity** — less code wins if perf is equal

When you pick one approach over another for perf reasons, document it — don't just leave a fast impl with no context.

---

## Git & GitHub

### Repos

Separate repo per component:
- `minemares-cruiser`
- `minemares-internode`
- `minemares-stomzy`

### Commits

Imperative, lowercase, plain. No scope prefixes, no caps, no punctuation.

```
fix packet flush on combat path
add spear charge velocity scaling
remove legacy knockback fallback
```

Drop a body when the why or impact isn't obvious from the title:

```
add spear charge velocity scaling

- scales 0.8x to 1.4x based on hold duration
- dead zone skips calc under 0.2s
- reads charge_start from spacetimedb
```

Body rules:
- dashes, lowercase, tight
- only what the title doesn't already say
- repro steps on bugs

### Branches

`{component}/{what}` — kebab, no caps:

```
stomzy/spear-charge
internode/flush-fix
cruiser/signal-handling
stomzy/bard-aura
```

### Merge Strategy

Squash merge always. One clean commit per feature on main, messy branch history disappears.

### PRs

Minimal. Just what the title doesn't cover. Repro steps on bugs. No templates, no headers.

### Issues

Informal titles only. No label taxonomy, no templates. Title speaks for itself.

---

## Cross-Language Conventions

### Comments

**Placement:** same line, after the code.

**Voice:** lowercase, no period, terse. Short words, modern vocab. `auto` not `automatically`. Condense everything.

```rust
let v = (v - 0.04) * 0.98; // drag before gravity, vanilla 1.8 order
player.skip_kb = true;      // intentional — bards don't eat their own KB
```

**When to comment:**
- Logic is non-obvious
- The *why* behind a decision needs to be on record — tradeoffs, intentional choices, things that look wrong but aren't

Don't comment obvious code. Don't explain what the code does — explain why it does it that way.

**Doc comments:** full rustdoc / godoc / javadoc on public API. Terse, no name-repeat, no filler.

```go
// launches internode proxy, auto respawns on non-zero exit unless ctx cancelled
func spawnInternode(cfg Config) error {
```

```rust
/// flushes bulk queue, skips combat-priority packets — those go direct
fn flush_bulk_queue(&mut self) -> Result<()> {
```

**Dead code:** always deleted. Ghost code pending replacement stays with a `DEAD` tag.

**Long functions:** never use section dividers — break into smaller functions instead.

---

### Tags

All tags carry your handle. Hot issues get `!`.

```
TODO(max):    work needed, not urgent
FIXME(max):   broken, needs a fix
FIXME!(max):  broken, will blow up in prod
HACK(max):    works but dirty, revisit
PERF(max):    explains a perf tradeoff or optimization decision
DEAD(max):    ghost code, not fully replaced yet
SAFE(max):    intentional unsafe block, explains the invariant
NET(max):     networking-specific tech debt
TEMP(max):    placeholder until a system ships
TESTING(max): stress test hook, remove before prod
POLISH(max):  works but rough, clean up post-launch
```

Examples in context:

```rust
// PERF(max): off-heap alloc, avoids GC pressure under 20v20
let buf = PooledByteBufAllocator.DEFAULT.buffer();

// FIXME!(max): drops packets when queue exceeds 1024 under load

// SAFE(max): ptr non-null, bounds checked by BoltFFI init contract
unsafe { ffi::bolt_send(ptr, len) }

// TEMP(max): hardcoded until bard aura config ships
const AURA_RADIUS: f32 = 5.0;
```

---

### Naming — Shared Rules

**Variables:** short but readable. Context carries the weight.
```
player, vel, kb, buf, pkt, conn, tick
```
Not `p`, `v`, `x` — but not `knockbackVelocityVector` either.

**Booleans:** plain, no prefix.
```
alive, dead, buffed, flushed, connected
```
Not `isAlive`, `hasBuffs`.

**Acronyms:** all caps, always.
```
calcKB, BoltFFI, AOI, TPS, FFI, NPC
```

**Constants:** SCREAMING_SNAKE across all languages.
```
SPEAR_REACH, DRAG_COEFF, MAX_PLAYERS, KB_SCALE
```

**Functions:** full words, verb first.
```
flush_packet_queue()
calculate_knockback()
spawn_internode()
register_combat_hit()
```

---

### Null / Nil / Absent State

Null is banned everywhere possible. Absent state is modeled as state, not null.

- **Rust:** `Option<T>` explicit, never assume `Some`
- **Kotlin:** non-null types enforced, `!!` is banned
- **Go:** zero values preferred, nil only on interfaces, pointers, and errors
- **Java:** `Optional<T>` on return types, never return null

---

### Hot Paths

Mark combat-path and per-tick functions explicitly:

```rust
// hot path — no alloc, no lock
fn process_combat_tick(&mut self, p: &Player) -> Result<()> {
```

```kotlin
// hot path — no alloc, no suspension
fun applyKnockback(p: Player, vel: Float) {
```

---

## Go — CRUISER

### Naming

| Construct | Convention |
|---|---|
| Exported types/funcs | `PascalCase` |
| Unexported | `camelCase` |
| Acronyms | All caps — `TPS`, `FFI`, `AOI` |
| Constants | `SCREAMING_SNAKE` |
| Files | `snake_case.go` |

```go
type PacketQueue struct{}
func (q *PacketQueue) FlushAll() error {}
var tpsLimit = 20
const MAX_PLAYERS = 500
```

### Collections

Pre-sized, access pattern drives the choice:

```go
// ordered / sequential
queue := make([]Packet, 0, 256)

// identity / key lookup
players := make(map[string]Player, 64)
```

### Error Handling

Sentinel errors for known states, wrapping for propagation — both:

```go
var ErrProcessDead = errors.New("process exited unexpectedly")

if err := spawnInternode(cfg); err != nil {
    return fmt.Errorf("internode spawn failed: %w", err)
}
```

### Goroutines & Concurrency

Shutdown via context + explicit done channel:

```go
go watchInternode(ctx, done, cfg)
```

Context cancellation triggers graceful exit. Done channel signals resource cleanup complete. Never leave a goroutine without a shutdown path.

### Process Lifecycle

CRUISER owns spawn, monitor, and reap of all child processes. Each child gets:
- its own context derived from root
- a done channel
- a respawn policy (backoff on non-zero exit, clean exit on signal)

---

## Rust — Internode (forked from Infrarust)

### Naming

| Construct | Convention |
|---|---|
| Types / Traits | `PascalCase` |
| Functions / Variables | `snake_case` |
| Constants | `SCREAMING_SNAKE` |
| Modules | `snake_case` |
| Files | `snake_case.rs` |

```rust
struct PacketPipeline;
fn flush_packet_queue() {}
const MAX_PLAYERS: u32 = 500;
mod priority_channel;
```

### Collections

Match structure to access pattern, always pre-allocate:

```rust
// sequential drain
let queue: VecDeque<Packet> = VecDeque::with_capacity(256);

// id lookup
let players: HashMap<u64, Player> = HashMap::with_capacity(64);

// default accumulator
let buf: Vec<u8> = Vec::with_capacity(1024);
```

### Error Handling

`thiserror` enums per domain:

```rust
#[derive(thiserror::Error, Debug)]
enum InternodeError {
    #[error("pipeline flush failed: {0}")]
    FlushFailed(String),
    #[error("player not found: {id}")]
    PlayerNotFound { id: u64 },
}
```

### Tokio Async

Named tracing span + comment above every spawn:

```rust
// flush worker — drains bulk queue on 50ms interval
let _span = tracing::info_span!("flush_worker").entered();
tokio::spawn(flush_worker(rx));
```

Shutdown via context cancellation signal through task tree.

### Unsafe Policy

Allowed at FFI boundaries and proven hot-path micro-optimizations where bounds are checked upstream.

Always wrapped in a safe abstraction, never exposed raw. `SAFE(max):` tag inside with the invariant:

```rust
fn send_bolt_packet(pkt: &Packet) -> Result<()> {
    // SAFE(max): ptr non-null, len bounds checked before call
    unsafe { ffi::bolt_send(pkt.as_ptr(), pkt.len()) }
    Ok(())
}
```

### Crate Workspace Structure

Internode follows Infrarust's multi-crate workspace layout. Protocol logic is isolated from proxy logic:

```
internode/
├── crates/
│   ├── internode_core/       # proxy pipeline, routing, connection pool
│   ├── internode_protocol/   # minecraft packet encode/decode
│   └── internode_plugin/     # plugin trait + built-in plugins
├── Cargo.toml                # workspace root
```

Each crate has one job. `internode_protocol` never imports `internode_core`. Cross-crate deps go downward only.

### Proxy Modes

Internode inherits Infrarust's proxy mode model. Each mode has a different packet handling contract — make sure the mode is obvious at the call site:

```rust
// mode determines how deep we go into the pipeline
match conn.mode {
    ProxyMode::Passthrough => forward_raw(conn),    // zero-copy, no decode
    ProxyMode::ZeroCopy   => forward_zero_copy(conn),
    ProxyMode::ClientOnly => handle_client_auth(conn),
    ProxyMode::Offline    => handle_offline(conn),
    ProxyMode::ServerOnly => handle_server_auth(conn),
}
```

Zero-copy is the default target for passthrough — never decode what you don't have to touch.

### Telemetry Feature Flag

Telemetry (OpenTelemetry) is opt-in via feature flag:

```toml
[features]
telemetry = ["opentelemetry", "metrics-exporter-prometheus"]
```

Hot-path code must not call telemetry behind a feature flag without a `#[cfg(feature = "telemetry")]` guard. Never let a disabled telemetry feature add overhead to the combat path.

### FFI — BoltFFI / Panama

Shared off-heap via Panama `MemorySegment`. Zero-copy — Rust writes directly into pre-registered off-heap memory, Java reads without crossing heap boundary. No GC involvement on the data path.

```rust
// SAFE(max): MemorySegment pre-registered, lifetime pinned for call duration
unsafe {
    let slice = std::slice::from_raw_parts_mut(seg_ptr, seg_len);
    write_packet(slice, &pkt);
}
```

---

## Kotlin / Java — Stomzy (forked from Minestom)

### Minestom Thread Architecture — Must Know

Stomzy inherits Minestom's multi-threaded tick model. Ticks are split into batches, each assigned to a thread from the `ThreadDispatcher` pool. Every entity knows its tick thread before the tick starts — this is what makes the Acquirable API efficient.

**This means entity state is NOT safe to access from arbitrary threads.** The Acquirable API is how you cross that boundary safely. Treat it as a hard rule, not a suggestion.

### Acquirable API — Hard Rules

Minestom's `Acquirable<T>` is the core thread-safety primitive in Stomzy. These rules are non-negotiable:

**Rule 1 — Never store raw entity refs. Always store `Acquirable<T>`:**

```java
// NEVER do this
private Player player; // raw ref — not thread safe

// always this
private Acquirable<Player> player; // safe to hold across threads
```

**Rule 2 — `sync()` blocks, `async()` defers to end of tick. Pick deliberately:**

```java
// sync — blocks current thread until entity's thread is free
// use when you need the result before continuing
acquirableEntity.sync(entity -> {
    entity.setHealth(entity.getHealth() - damage);
});

// async — schedules for end of tick, non-blocking
// use when you don't need the result immediately
acquirableEntity.async(entity -> {
    entity.sendMessage(Component.text("You were hit"));
});
```

**Rule 3 — Never use the acquired object outside the consumer:**

```java
acquirableEntity.sync(entity -> {
    float hp = entity.getHealth(); // fine inside
    // DON'T return entity out of this lambda
});
```

**Rule 4 — `AcquirableCollection` — never naive loop:**

```java
// NEVER do this — acquires one at a time, massive overhead
for (Acquirable<Player> p : players) {
    p.sync(player -> { ... });
}

// always this — optimal batch acquisition
players.acquireSync(player -> { ... });
players.acquireAsync(player -> { ... });
```

**Rule 5 — `unwrap()` is unsafe. Must carry a `SAFE(max):` tag:**

```java
// SAFE(max): called from entity's own tick thread, no cross-thread access
Entity entity = acquirableEntity.unwrap();
```

If you can't write a clear invariant for why it's safe — don't unwrap.

### Thread-Safe Fields

Follow JVM thread-safety rules for any field accessed across threads:

```java
// shared mutable state — volatile ensures visibility across threads
private volatile boolean alive = true;

// immutable — final is free thread safety, prefer it always
private final UUID id;

// high-frequency shared counters — atomic over synchronized
private final AtomicInteger hitCount = new AtomicInteger(0);
```

### Thread-Safe Collections

Never use bare `HashMap` or `ArrayList` for shared state:

```java
// player registry, entity maps — concurrent
private final Map<UUID, Acquirable<Player>> players = new ConcurrentHashMap<>();

// event queues, packet queues — concurrent linked
private final Queue<CombatEvent> events = new ConcurrentLinkedQueue<>();

// read-heavy, rarely mutated — copy-on-write
private final List<Buff> passiveBuffs = new CopyOnWriteArrayList<>();
```

### Entity Hierarchy

Minestom's entity class hierarchy — pick the right base, don't over-extend:

```
Entity              → spawning, metadata, basic physics — lowest overhead
 └─ LivingEntity    → health, equipment, attributes
     └─ EntityCreature → navigation, AI goals — only when you need pathfinding
ItemEntity          → dropped items, extends Entity directly
```

For combat NPCs use `EntityCreature`. For static/visual entities use `Entity`. Never extend `Player` for NPCs — use `Mannequin` (1.21.9+) or the `Entity`-based approach.

### Batch Metadata Updates

Every metadata call sends a packet to all viewers. Batch them:

```java
// PERF(max): batch meta — 1 packet to viewers instead of N
EntityMeta meta = entity.getEntityMeta();
meta.setNotifyAboutChanges(false);
meta.setOnFire(true);
meta.setCustomNameVisible(true);
meta.setCustomName(Component.text("TARGET", NamedTextColor.RED));
meta.setNotifyAboutChanges(true); // fires single batched packet here
```

Always batch when modifying more than one metadata field on a spawned entity.

### Instance System

Stomzy uses Minestom's Instance system, not worlds. Instances are lightweight — they can live in memory only, copy fast, and tick independently:

```java
// each game arena = its own instance
InstanceContainer arena = MinecraftServer.getInstanceManager().createInstanceContainer();
```

Instances are the unit of isolation for 20v20 fights. One match = one instance. Never share an instance between unrelated matches.

### Schedulers

Use Minestom's built-in scheduler for deferred/repeating game logic. Don't roll your own tick loops:

```java
// schedule on entity's tick thread — no acquisition needed
entity.scheduler().buildTask(() -> {
    entity.setGlowing(false); // safe — runs on entity's thread
}).delay(TaskSchedule.tick(20)).schedule();
```

### Naming

| Construct | Convention |
|---|---|
| Classes / Objects | `PascalCase` |
| Functions / Properties | `camelCase` |
| Constants | `SCREAMING_SNAKE` |
| Files | `PascalCase.kt` |

```kotlin
data class PlayerState(val alive: Boolean)
fun calculateKnockback(): Float {}
object PacketRegistry {}
const val MAX_REACH = 4.5f
```

### Collections

Immutable snapshots, mutable state containers:

```kotlin
// data passed around = immutable snapshot
data class PlayerState(val buffs: List<Buff>)

// state managed over time = mutable container
class BuffTracker {
    val active = mutableListOf<Buff>()
}
```

### Error Handling

`Result` on IO/network boundaries, exceptions on logic violations:

```kotlin
fun loadPlayerState(id: UUID): Result<PlayerState> =
    runCatching { spacetimeDB.query(id) }

fun calculateKnockback(vel: Float): Float {
    require(vel >= 0f) { "negative velocity" }
    return vel * KB_SCALE
}
```

### Coroutines vs Netty

Netty owns all packet I/O — encode, decode, flush. Coroutines own game logic and SpacetimeDB calls. Never suspend inside a Netty handler. Coroutines are used where they genuinely help, not avoided religiously.

```kotlin
// Netty handler — no suspension
class CombatPacketHandler : ChannelInboundHandlerAdapter() {
    override fun channelRead(ctx: ChannelHandlerContext, msg: Any) {
        val pkt = msg as CombatPacket
        combatScope.launch { processCombatHit(pkt) } // hand off to coroutine
    }
}
```

---

### Kotlin / Java Naming

**Kotlin:**

| Construct | Convention |
|---|---|
| Classes / Objects | `PascalCase` |
| Functions / Properties | `camelCase` |
| Constants | `SCREAMING_SNAKE` |
| Files | `PascalCase.kt` |

**Java:**

| Construct | Convention |
|---|---|
| Classes | `PascalCase` |
| Methods / Variables | `camelCase` |
| Constants | `SCREAMING_SNAKE` |
| Files | `PascalCase.java` |

Java and Kotlin are full equals in Stomzy. Java handles Minestom API extension, entity subclassing, and Panama interop. Kotlin handles game logic, SpacetimeDB calls, and coroutine scopes. Neither is a wrapper for the other.

---

## Networking & Packet Conventions

### Packet Naming

Packets named by direction and domain:

```
CombatHitPacket       // inbound combat
KnockbackPacket       // outbound state
PlayerPositionPacket  // inbound movement
ChunkDataPacket       // outbound bulk
```

### Priority Annotation

Priority lives on the packet or handler, not scattered in logic:

```kotlin
val priority = PacketPriority.COMBAT    // immediate dispatch
val priority = PacketPriority.HIGH      // inventory, health
val priority = PacketPriority.BULK      // chunks, spawns
```

Combat and position = immediate. Inventory/health = high. Chunks/spawns = bulk.

### Latency Compensation

Lag-aware code paths are always marked:

```kotlin
// lag compensation — tick delta applied to charge_start for accurate hold duration
val holdDuration = currentTick - chargeStart - lagDelta
```

---

## SpacetimeDB Conventions

### Reducer Naming

Domain prefixed, verb second:

```
combat_apply_knockback
combat_register_hit
player_update_position
player_update_state
net_register_connection
net_drop_player
bard_apply_aura
```

### Table Design

Hot columns (combat state, position, velocity) in their own tables. Cold columns (cosmetics, settings, history) separated. Never mix read-hot and write-cold in the same table.

### Shared State Policy

SpacetimeDB is truth for anything latency-critical or shared across players. Local state is ephemeral and only used for non-critical, non-shared data that doesn't need to be fast or durable.

---

## Logging & Telemetry

### Log Levels

| Level | Use |
|---|---|
| `error` | Unrecoverable, needs attention now |
| `warn` | Degraded but alive, investigate soon |
| `info` | Lifecycle events only — spawn, shutdown, connect |
| `debug` | Packet flow, state changes |
| `trace` | Per-tick, per-player — never in prod |

### What Never Gets Logged

Per-tick and per-packet events never hit log files. That data goes to Hyperscope metrics only. Logs are for lifecycle and failure — not volume.

### Hyperscope Metrics

All performance telemetry — TPS, packet queue depth, entity suppression rate, chunk cache hits, JVM heap, priority channel throughput — routes to Hyperscope. If it's a number that changes over time, it's a metric, not a log line.
