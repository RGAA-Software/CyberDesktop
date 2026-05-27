# CyberEditor — feature roadmap & execution backlog

**Product:** Notepad++-class editor (local files, find/replace, tabs).  
**Engine:** vendored Zed `editor` + `language` + `grammars` (`crates/editor/`).  
**Chrome:** `gpui-component` (toolbar, status, find strip — not the text surface).

**Verify build / run (use CyberEditor, not CyberFiles):**

```powershell
cargo cybereditor
cargo cybereditor-run -- path\to\file.rs
```

Manual QA: [cybereditor-manual-qa.md](cybereditor-manual-qa.md)

Master phase doc: [cybereditor-implementation-phases.md](cybereditor-implementation-phases.md).

---

## Current snapshot (2026-05-27)

| Area | Status |
|------|--------|
| Zed `Editor` embedded (`cybereditor` + `zed-engine`) | done |
| Open / Save / Save As, dirty title, status bar | done |
| Find / replace / comment / indent (engine + modal UI) | done |
| Syntax highlighting (tree-sitter) | **done (A1)** — verify with `cargo cybereditor-run -- file.rs` |
| Save clears engine dirty (A4) | **done** — verify title `*` after save |
| Find strip (no modal) | pending (C1) |
| Multi-tab | pending (D1) |

---

## Stage A — Engine usable (finish Phase 0c)

**Goal:** Code files look like an editor; large files stay responsive.

| ID | Task | Status | Acceptance |
|----|------|--------|------------|
| A1 | `LanguageRegistry` + `grammars` subset; wire `Buffer` language | done | `.rs` / `.py` colored |
| A2 | Open/switch file syncs language + `set_document` | done | Path passed into `set_document` / `set_highlighter` |
| A3 | ~20 MB scroll/type benchmark | pending | Usable; no LSP process |
| A4 | After save: `Buffer::did_save` + session dirty | done | Title `*` clears |

**Exit:** `cargo cybereditor-run` + syntax + large-file smoke test.

---

## Stage B — Production engine (Phase 1)

| ID | Task | Acceptance |
|----|------|------------|
| B1 | `zed-engine` only; remove / isolate `ModelEditorBackend` | No dead backend warnings |
| B2 | Trim `EditorBufferModel`; cursor/find from engine | Single source of truth |
| B3 | Verify fold, bracket match on Zed path | Manual checklist |
| B4 | Root `LICENSE` / `NOTICE` for GPL vendored tree | Release gate |

---

## Stage C — Notepad++ UX (Phase 2)

| ID | Task | Acceptance |
|----|------|------------|
| C1 | Bottom find/replace strip (`gpui-component`) | Ctrl+F no alert |
| C2 | Find options UI (case, whole word) | Options work |
| C3 | Match count in status (`n/m`) | Visible while finding |
| C4 | External file change → reload prompt | External edit detected |
| C5 | Native file dialogs for Open/Save | Better than path alert |

---

## Stage D — Multi-document (Phase 3)

| ID | Task | Acceptance |
|----|------|------------|
| D1 | Tab model (path + buffer/editor per tab) | Switch files |
| D2 | Unsaved tab close guard | Same as today |
| D3 | Optional: embed in CyberFiles shell | Double-click opens tab |

---

## Stage E — Polish (Phase 4)

Recent files, drag-drop open, EOL on save, regex find UI, optional non-UTF-8, large-file open warning.

---

## Explicitly out of scope

LSP, diagnostics, completion, goto-def, git gutter, agent, terminal, Zed workspace UI, full `languages` crate (node toolchains).

---

## Recommended order

```text
A1 → A4 → A2 → A3 → B1 → B2 → C1 → C4 → D1 → E*
```

---

## Key source files

| Area | Path |
|------|------|
| Page / chrome | `crates/ui/src/cyber_editor/page.rs` |
| Zed backend | `crates/ui/src/cyber_editor/zed_backend.rs` |
| Host API | `crates/ui/src/cyber_editor/editor_host.rs` |
| Engine glue | `crates/cyber-editor-engine/` |
| Ext → language id | `crates/ui/src/cyber_editor/language.rs` |
| Vendored engine | `crates/editor/` |
