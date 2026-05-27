# CyberEditor — manual QA checklist

Run the editor binary (not CyberFiles):

```powershell
cargo cybereditor-run
# or with a file:
cargo cybereditor-run -- path\to\file.rs
```

## P0 — Text input (must pass before next features)

| # | Action | Expected |
|---|--------|----------|
| 1 | Click in text area | Caret visible; status bar line/col updates |
| 2 | Type `hello` | Characters appear |
| 3 | Arrow keys | Caret moves |
| 4 | Home / End | Line start / end |
| 5 | Backspace / Delete | Deletes char |
| 6 | Ctrl+A, Ctrl+C, Ctrl+V | Select all, copy, paste |
| 7 | Undo / Redo (Ctrl+Z / Ctrl+Y) | Works |

## P0 — Chrome shortcuts (with editor focused)

| # | Action | Expected |
|---|--------|----------|
| 8 | Ctrl+S | Saves (or Save As if untitled) |
| 9 | Ctrl+O | Open file dialog |
| 10 | Ctrl+F | Find dialog |

## P1 — Session

| # | Action | Expected |
|---|--------|----------|
| 11 | Edit + save | Title `*` clears |
| 12 | Open `.rs` file | Syntax highlighting |
| 13 | Close with unsaved changes | Prompt |

Record failures with OS build and file type.
