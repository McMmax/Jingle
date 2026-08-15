# 🔔 Jingle

**Jingle** is a lightning-fast Rust templating and styling engine built specifically for formatting Minecraft chat and UI elements.

---

## 🎯 What does it do?
Minecraft's chat formatting (MiniMessage, legacy color codes, JSON components) can be notoriously messy to deal with programmatically. 

Jingle provides a clean, declarative template engine. You write UI components or chat messages as templates, and Jingle rapidly parses, styles, and renders them into network-ready packets or strings. 

## 🏗️ Architecture

- **`template/`**: The core parser and evaluator that turns raw strings into structured template ASTs.
- **`style.rs` & `variants.rs`**: Handles the actual styling (colors, bold, italics, click events) and component variations.
- **`catalog.rs`**: A localized storage registry for holding reusable UI component templates.

## 🚀 Key Features
- **Blazing Fast**: Written entirely in Rust. It can render thousands of complex dynamic chat messages per tick with virtually zero CPU strain.
- **Component Based**: Build small, reusable UI components (like a `[Prefix]` or a `Button`) and compose them together.
- **Placeholder Support**: Automatically evaluates logic and replaces dynamic variables on the fly.
