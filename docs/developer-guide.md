# 🔔 Jingle: High-Speed Native Templating & Chat Engine

> **SIMD-Accelerated String Interpolation, Color Formatting, and Chat Component Serializer in Rust.**  
> *Hard-proof documentation for `libs/jingle`.*

---

## ⚡ 1. Overview & Architecture

`jingle` is a pure-Rust templating engine exposing C-ABI bindings (`cdylib`) and Rust APIs. It replaces heavy JVM regex string formatters with zero-allocation SIMD byte searching.

---

## 📋 2. Verified Rust Usage

```rust
use jingle::Template;

let template = Template::compile("§6{player} §7killed §c{victim} §7with §e{weapon}");

let mut vars = std::collections::HashMap::new();
vars.insert("player", "Notch");
vars.insert("victim", "Alex");
vars.insert("weapon", "Diamond Sword");

let formatted = template.render(&vars);
assert_eq!(formatted, "§6Notch §7killed §cAlex §7with §eDiamond Sword");
```

---

## 📖 Canonical Specifications
* **[`.bible/09-shared-libraries-and-apis`](file:///C:/Users/trapaholics/Documents/GitHub/.bible/09-shared-libraries-and-apis)**
