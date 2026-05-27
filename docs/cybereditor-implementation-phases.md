# CyberEditor engine — phased implementation

Vendored upstream editor crates live under **`crates/editor/`** (directory name `editor`, not `zed`). The Rust package for the view is still named `editor` at `crates/editor/editor/`.

**Feature backlog (detailed tasks):** [cybereditor-feature-roadmap.md](cybereditor-feature-roadmap.md)

**Build / verify command (use this, not full CyberFiles):**

```powershell
cargo build -p cyberfiles --bin cybereditor --features zed-engine
# shorthand (see .cargo/config.toml):
cargo cybereditor
cargo cybereditor-run -- path\to\file.rs
# or, while working only on the vendored workspace:
cargo check --manifest-path crates/editor/Cargo.toml -p <crate>
```

---

## Phase 0a — Infrastructure ✅ (2026-05-27)

| Task | Status |
|------|--------|
| `docs/cybereditor-implementation-phases.md` | done |
| `docs/cybereditor-code-editor-plan.md` → `crates/editor/` | done |
| `crates/editor/README.md`, `manifest-tiers.toml`, `UPSTREAM` | done |
| `scripts/sync_editor_from_upstream.ps1` | done |
| Tier 0 sync + nested `crates/editor/Cargo.toml` | done |
| `cargo check --manifest-path crates/editor/Cargo.toml -p text` | **pass** |

---

## Phase 0b — Editor stack compile closure ✅

| Task | Status |
|------|--------|
| Sync Tier 1 (`language*`, `multi_buffer`, `settings*`, `theme*`, `grammars`) | done |
| Sync Tier 2 (`component`, `ui*`, `menu`, `snippet`, `markdown`, `editor`) | done |
| Sync Tier 3 (closure expansion list in `manifest-tiers.toml`) | done |
| `cargo check --manifest-path crates/editor/Cargo.toml -p editor` | **pass** |

Current snapshot:

- Vendored members: **91**
- `editor` crate can compile in isolated vendored workspace.

**Exit:** vendored `editor` crate compiles in isolation.

---

## Phase 0c — Spike in `cybereditor` binary 🚧

| Task | Status |
|------|--------|
| `scripts/merge_editor_workspace_into_root.ps1` | done |
| `crates/cyber-editor-engine` glue crate | done |
| `ZedEditorBackend` + `zed-engine` feature on `cyberfiles-ui` | done |
| `cybereditor` binary `required-features = ["zed-engine"]` | done |
| Root workspace merge | done |
| `cargo build -p cyberfiles --bin cybereditor --features zed-engine` | **pass** |
| **A1** Syntax: `LanguageRegistry` + grammars → `Buffer` language | done |
| **A4** Save → `Buffer::did_save` / dirty sync | done |
| **P0 keyboard** Default keymap + editor focus (arrows, backspace, typing) | done (see [cybereditor-manual-qa.md](cybereditor-manual-qa.md)) |
| **A3** Open 20 MB file benchmark vs `InputState` | pending |

**Exit:** `cargo cybereditor-run`; syntax on `.rs`; 20 MB scroll/type acceptable; no LSP process.

---

## Phase 1 — Production swap

| Task | Status |
|------|--------|
| Wire root workspace: `cyberfiles-ui` depends on `editor` path | done (via feature) |
| `EditorHost` default = engine backend; remove `ModelEditorBackend` | pending |
| Dirty / save / status from `Buffer` | partial (A4) |
| Editor flags: no minimap, git gutter, code actions, edit predictions | done |

**Exit:** daily editing on engine only; GPL `NOTICE` at repo root.

---

## Phase 2 — Notepad++ UX

| Task | Status |
|------|--------|
| Find/replace strip (gpui-component), drop modal dialogs | pending |
| External file reload prompt | pending |
| `apply_edit` helper for remaining commands | done (Zed path) |

---

## Phase 3 — Multi-tab

| Task | Status |
|------|--------|
| Tab model + tab bar; one buffer/editor per tab or shared editor | pending |

---

## Phase 4 — Polish

Recent files, drag-drop, encoding/EOL on save, regex find.

---

## Execution order (2026-05-27)

```text
A1 syntax → A4 save/dirty → A2 language on open → A3 benchmark
→ B1 remove InputState backend → C1 find strip → C4 file watch → D1 tabs
```

See [cybereditor-feature-roadmap.md](cybereditor-feature-roadmap.md) for full task IDs.

---

## Crate tiers (`manifest-tiers.toml`)

See `crates/editor/manifest-tiers.toml` for copy lists per phase.

## License

Vendored `crates/editor/*` (GPL portions): add root `LICENSE` / `NOTICE` before release (Phase 1 gate).
