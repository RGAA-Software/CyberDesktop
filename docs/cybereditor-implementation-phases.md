# CyberEditor engine — phased implementation

Vendored upstream editor crates live under **`crates/editor/`** (directory name `editor`, not `zed`). The Rust package for the view is still named `editor` at `crates/editor/editor/`.

**Build / verify command (use this, not full CyberFiles):**

```powershell
cargo build -p cyberfiles --bin cybereditor --features zed-engine
# shorthand (see .cargo/config.toml):
cargo cybereditor
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

\* Only crates required by `cargo tree -p editor`.

Current snapshot:

- Vendored members: **91**
- `editor` crate can compile in isolated vendored workspace.
- Script status: `scripts/sync_editor_from_upstream.ps1` now supports
  - tier-based member generation
  - preserving `workspace.lints`
  - preserving `workspace.dependencies.<subtable>` (e.g. `windows`)
  - overriding `gpui*`, `gpui_shared_string`, `http_client`, `refineable` to upstream git when needed for type identity
  - stripping `dev-dependencies` in vendored crates
  - The vendored ui/editor code currently has compatibility fixes for `BoxShadow { inset }` against current GPUI API.

**Exit:** vendored `editor` crate compiles in isolation.

---

## Phase 0c — Spike in `cybereditor` binary 🚧

| Task | Status |
|------|--------|
| `scripts/merge_editor_workspace_into_root.ps1` | done |
| `crates/cyber-editor-engine` glue crate | done |
| `ZedEditorBackend` + `zed-engine` feature on `cyberfiles-ui` | done |
| `cybereditor` binary `required-features = ["zed-engine"]` | done |
| Root workspace merge (`merge_editor_workspace_into_root.ps1`) | done |
| `cargo build -p cyberfiles --bin cybereditor --features zed-engine` | **pass** |
| Open 20 MB file benchmark vs `InputState` | pending |

**Exit:** `cargo build -p cyberfiles --bin cybereditor --features zed-engine`; 20 MB scroll/type acceptable; no LSP process.

---

## Phase 1 — Production swap

| Task | |
|------|--|
| Wire root workspace: `cyberfiles-ui` depends on `editor` path | |
| `EditorHost` default = engine backend; remove `ModelEditorBackend` | |
| Dirty / save / status from `Buffer` | |
| Editor flags: no minimap, git gutter, code actions, edit predictions | |

**Exit:** `cargo build -p cyberfiles --bin cybereditor`; daily editing on engine.

---

## Phase 2 — Notepad++ UX

| Task | |
|------|--|
| Find/replace strip (gpui-component), drop modal dialogs | |
| External file reload prompt | |
| `apply_edit` helper for remaining commands | |

---

## Phase 3 — Multi-tab

| Task | |
|------|--|
| Tab model + tab bar; one buffer/editor per tab or shared editor | |

---

## Phase 4 — Polish

Recent files, drag-drop, encoding/EOL on save, regex find.

---

## Crate tiers (`manifest-tiers.toml`)

See `crates/editor/manifest-tiers.toml` for copy lists per phase.

## License

Vendored `crates/editor/*` (GPL portions): add root `LICENSE` / `NOTICE` before release (Phase 1 gate).
