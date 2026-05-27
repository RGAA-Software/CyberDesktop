# Files Parity Priority Backlog

This document replaces the older parity notes. Keep `dependency-policy.md` as the only standing policy doc, and use this file as the execution backlog.

## Priority Order

### P0: Info Pane And Preview

Reason:
- This is the most obvious product gap versus `Files`.
- It improves both Home and file browsing flows without changing navigation architecture.
- It raises perceived product quality faster than command-palette work.

Scope:
- Expand preview type handling beyond image + plain text.
- Add better preview rendering for code, Markdown, HTML, SVG, PDF, and media where feasible.
- Keep details and preview behavior consistent across single-pane and dual-pane.
- Later, evaluate shell preview handlers or external preview popup integration.

Execution order:
1. Improve preview classification and rendering for image, SVG, code, Markdown, HTML, and plain text.
2. Add PDF preview support if a lightweight in-process option exists.
3. Add media preview support.
4. Evaluate shell preview handler hosting and external preview popup integration as a separate track.

### P1: Recycle Bin Completeness

Reason:
- Current browsing exists, but the action set is incomplete.
- The missing actions are narrow and high value.

Scope:
- Restore selected recycle-bin items.
- Empty recycle bin.
- Recycle-bin-specific toolbar and context menu behavior.

### P2: Shell-Level Polish

Reason:
- The shell structure is already strong, but a few workflows still feel unfinished.

Scope:
- Strengthen status bar behavior.
- Revisit file browser interaction rough edges as they appear.
- Fill gaps in selection, focus, and pane synchronization.

### P3: Omnibar Command Mode

Reason:
- Useful for discoverability and keyboard-heavy usage.
- Not a blocker for core file-manager completeness.
- More valuable after the major feature gaps above are closed.

Scope:
- Lightweight command registry.
- Omnibar command mode with suggestion list and execution.
- Shortcut display and context-aware command enablement.

### P4: Advanced Integrations

Reason:
- High value for power users, but not core parity blockers.

Scope:
- Git branch and sync status in the shell.
- QuickLook / Seer / Peek style external preview integration.
- Broader cloud-drive and shell-specific metadata polish.

## Current Step

Start with `P0.1`: improve preview classification and rendering for image, SVG, code, Markdown, HTML, and plain text.
