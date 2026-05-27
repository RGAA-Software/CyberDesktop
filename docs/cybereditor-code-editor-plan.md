# CyberEditor Code Editor Plan

CyberEditor is a **Notepad++-class text editor** (local files, syntax highlighting, find/replace, tabs) with a **Zed-grade editing engine** for **large-file performance**.

**Last re-evaluation:** 2026-05-27 — product = **Notepad++**; render/edit core = **Zed `editor`** (trigger: ~20 MB text is usable in Zed, unusably slow in `gpui-component` `InputState`).

---

## Product vs engine (two layers)

| Layer | Choice | Notes |
|-------|--------|--------|
| **Product** | Notepad++ | No LSP, no agent/collab/terminal/git IDE |
| **Engine** | Zed `Buffer` → `MultiBuffer` → `Editor` | Viewport-oriented display map, chunked syntax, rope + snapshots |
| **Chrome** | `gpui-component` | Toolbar, status bar, find strip, tabs, dialogs — **not** the text surface |

Do not confuse “use Zed editor” with “build Zed the IDE”. We vendor **editor + language/syntax**, not workspace UI or language servers.

---

## Why `gpui-component` lags on ~20 MB (observed)

Both sides use a **rope**, but cost dominates elsewhere:

| Area | `gpui-component` `InputState` | Zed `editor` + `language::Buffer` |
|------|------------------------------|-----------------------------------|
| Open / `set_value` | Replaces whole rope; code mode sets `_pending_update` → **full-buffer highlighter pass** | Buffer load + incremental syntax map (`MAX_BYTES_TO_QUERY`, row **chunks**) |
| Highlight / folds | tree-sitter on full text or heavy sync path; fold extraction walks syntax tree | Syntax map updated in **chunks**; highlight queries bounded |
| Layout / paint | `display_map` + layout for editor, but tied to input element’s update model | **`display_map`** pipeline: tab/wrap/fold maps; paint **visible** regions |
| Undo / edit | Rope + history in input state | `text::Buffer` transactions + buffer replicas |

So Notepad++-level **features** do not require Zed, but **20 MB+ smooth scroll/typing** is a validated reason to switch the **engine**.

**Trigger for Zed path: met** (user benchmark ~20 MB text).

---

## Notepad++ scope (unchanged)

| Include | Exclude |
|---------|---------|
| Open/save, dirty, encoding/EOL display | LSP, diagnostics, completion, goto-def |
| Syntax highlight (tree-sitter, no server) | Agent, collab, terminal, debugger |
| Find/replace, goto line, regex (later) | Git gutter, extensions host |
| Line numbers, fold, wrap, indent guides | Zed application workspace UI |
| Multi-tab (CyberFiles shell) | Full `languages` crate (node/toolchain) |

---

## Architecture

```text
CyberEditorPage
  gpui-component — toolbar, status, find strip, tabs, notifications
  EditorSession  — path, dirty, encoding, find strings (CyberFiles)
  EditorHost
    ZedEditorBackend (target)     — Entity<Editor> + Buffer
    ModelEditorBackend (legacy)   — InputState; remove after swap
```

```text
language::Buffer
  -> MultiBuffer::singleton
  -> Editor::new(EditorMode::full(), …, project: minimal | None)
```

- **Syntax:** `LanguageRegistry` + vendored **`grammars`** (subset). **Not** upstream `languages` crate.
- **Editor flags:** disable minimap, LSP-driven code actions, git diff gutter, edit predictions, inlay hints.
- **Files:** CyberFiles reads/writes disk; buffer is source of truth while editing.

---

## Implementation path: hybrid (Path D)

| Path | Role |
|------|------|
| ~~A. InputState only~~ | Fine for small files; **fails** large-file goal |
| **D. Zed engine + gpui-component chrome + N++ features** | **Selected** |
| B. Full Zed IDE vendoring | Rejected — too much product |

### Vendoring (`crates/editor/`)

Directory name is **`editor`** (not `zed`). The view package remains `crates/editor/editor/`.

Copy into repo (maintain in-tree, pin to same `gpui` git rev as root `Cargo.toml`):

**Tier A — engine:** `text`, `rope`, `clock`, `language`, `language_core`, `multi_buffer`, `editor`, `theme`*, `settings`*, `ui`*, `grammars` (subset)

**Tier B — link closure:** `project`, `workspace`, `worktree`, `fs`, `lsp` (types only; **no servers**), plus transitive deps until `cargo check -p editor` passes

**Do not copy:** `languages`, `agent*`, `collab*`, `terminal*`, `zed` binary, extension host

\* and small `settings_*` / `ui_*` siblings as required by `Cargo.toml`

**Size:** ~45–65 crates compiled; **product behavior** stays Notepad++-sized.

### License

Vendored editor/language stack: **GPL-3.0-or-later**. Accept for CyberEditor or obtain legal guidance before shipping. `gpui` / `gpui-component` remain Apache-2.0.

---

## Phases

### Phase 0 — Zed engine spike (priority)

1. GPL + `crates/editor/README.md` (upstream commit)
2. Import Tier A + B; `cargo check -p editor`
3. `ZedEditorBackend` + `--bin cybereditor` only
4. **Benchmark:** open ~20 MB `.txt` / `.log` — scroll and type; compare to current `InputState`
5. Attach one grammar (e.g. Rust or plain text); confirm highlight does not block UI

**Acceptance:** 20 MB file interactively usable; no `InputState` in spike; no LSP process.

### Phase 1 — Swap production editor

- `EditorHost` → `ZedEditorBackend` in `CyberEditorPage`
- Dirty/save/status from `Buffer` / `Editor` events
- Remove `ModelEditorBackend` and redundant `buffer_model` mirroring
- Large-file open policy (warn optional; engine should cope)

### Phase 2 — Notepad++ UX (on top of Zed)

- In-editor find/replace strip (gpui-component)
- External file change → reload prompt
- Central `apply_edit` for any remaining programmatic edits

### Phase 3 — Multi-tab

- Tab per path; each tab: `Buffer` + `Editor` (or one editor + swap buffer)
- CyberFiles tab bar; **not** Zed workspace UI

### Phase 4 — Polish

- Recent files, drag-drop, encoding/EOL on save, regex find

### Not planned

- LSP / IDE features (unless explicitly reopened)
- Optimizing `gpui-component` input for 20 MB as primary strategy (may still upstream small fixes)

---

## Acceptance criteria

- [ ] **Notepad++** feature set (no IDE)
- [ ] **~20 MB** text file: scroll + edit without unacceptable lag (vs current `InputState`)
- [ ] Core widget is Zed `Editor`, not `InputState`
- [ ] Syntax via `grammars` + registry, no language server
- [ ] App chrome remains `gpui-component`
- [ ] `EditorHost` single backend swap point
- [ ] GPL attribution complete for `crates/editor/`

---

## Current state

`crates/ui/src/cyber_editor/` — `EditorHost` + `ModelEditorBackend` (`InputState`), session/file commands, dialog find/replace.

**Next:** Phase 0c (`ZedEditorBackend` spike in `cybereditor`), since Phase 0b compile closure is complete. See [cybereditor-implementation-phases.md](cybereditor-implementation-phases.md).

**Verify build:** `cargo build -p cyberfiles --bin cybereditor` (not full `cyberfiles` unless needed).

---

## Optional: mitigate before Phase 0 completes

Short-term experiments on `InputState` (do **not** replace Zed plan):

- Open files &gt; N MB with **plain text** highlighter disabled
- Defer fold extraction on large buffers

These may help slightly; they do not match Zed’s display/syntax architecture and are not the long-term fix.

---

## References

- Notepad++: <https://notepad-plus-plus.org/>
- Zed embed: `zed/crates/inspector_ui/src/div_inspector.rs`
- CyberFiles: `crates/ui/src/cyber_editor/`
- [dependency-policy.md](dependency-policy.md) — in-repo `crates/zed/*` allowed
