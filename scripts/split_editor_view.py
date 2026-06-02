#!/usr/bin/env python3
"""One-shot splitter for editor_view/mod.rs into submodules."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "crates/ui-editor/src/editor_view/mod.rs"
EV = ROOT / "crates/ui-editor/src/editor_view"

# (relative_path, start_line, end_line) — 1-based inclusive
SECTIONS = [
    ("editor.rs", 60, 216),
    ("impl/core.rs", 218, 376),
    ("impl/movement.rs", 378, 439),
    ("impl/selection.rs", 440, 534),
    ("impl/tabs.rs", 541, 692),
    ("impl/close_confirm.rs", 694, 785),
    ("impl/recent.rs", 786, 795),
    ("impl/disk_watch.rs", 802, 845),
    ("impl/file_io.rs", 851, 1028),
    ("impl/clipboard.rs", 1029, 1057),
    ("impl/editing.rs", 1058, 1170),
    ("impl/goto.rs", 1171, 1209),
    ("impl/search_panel.rs", 1210, 1383),
    ("impl/find.rs", 1384, 1585),
    ("impl/keyboard.rs", 1586, 1740),
    ("impl/mouse_scroll.rs", 1741, 1904),
    ("ui/scrollbars.rs", 1905, 2101),
    ("ui/chrome.rs", 2105, 2342),
    ("ui/overlays.rs", 2343, 2598),
    ("ui/search_panel_ui.rs", 2599, 2774),
    ("ui/goto_bar.rs", 2775, 2818),
    ("ui/find_bar.rs", 2819, 2947),
    ("ui/widgets.rs", 2953, 3007),
    ("render.rs", 3015, 3137),
    ("input_handler.rs", 3141, 3309),
]

# CloseTarget lives between struct and impl — fold into editor.rs extract (133-137)
CLOSE_TARGET = (133, 137)

lines = MOD.read_text(encoding="utf-8").splitlines(keepends=True)


def slice_lines(start: int, end: int) -> str:
    return "".join(lines[start - 1 : end])


def write_impl(path: str, start: int, end: int, wrap: bool = True) -> None:
    body = slice_lines(start, end)
    out = EV / path
    out.parent.mkdir(parents=True, exist_ok=True)
    if path == "editor.rs":
        # struct + CloseTarget + impl block for constructors
        struct_part = slice_lines(61, 130)
        close_part = slice_lines(133, 137)
        impl_part = slice_lines(141, 216)
        content = (
            "//! `EngineEditor` entity and constructors.\n\n"
            + "use std::ops::Range;\n"
            + "use std::path::PathBuf;\n"
            + "use std::time::SystemTime;\n\n"
            + "use cyber_desktop_text_engine::{load_file, Document, SyntaxState};\n"
            + "use gpui::{px, App, Bounds, Context, Entity, FocusHandle, Pixels, Window};\n\n"
            + "use super::language::language_for_path;\n"
            + "use super::state::{\n"
            + "    FindState, InputTarget, SearchPanelState, TabSlot, VisibleLine, WrappedVisible,\n"
            + "};\n"
            + "use super::state::read_file_meta;\n\n"
            + struct_part
            + "\n"
            + close_part
            + "\n\nimpl EngineEditor {\n"
            + impl_part
            + "}\n"
        )
        # Fix struct visibility
        content = content.replace("pub struct EngineEditor {", "pub struct EngineEditor {")
        content = content.replace("\n    focus_handle:", "\n    pub(crate) focus_handle:")
        for field in [
            "document",
            "syntax",
            "parsed_revision",
            "marked_range",
            "is_selecting",
            "needs_focus",
            "input_target",
            "find",
            "goto",
            "search_panel",
            "show_line_numbers",
            "show_about",
            "show_shortcuts",
            "scrollbar_drag",
            "hscrollbar_drag",
            "reveal_caret",
            "font_size",
            "line_height",
            "gutter_width",
            "scroll_y",
            "scroll_x",
            "content_width",
            "last_bounds",
            "visible",
            "soft_wrap",
            "wrap_top_line",
            "wrap_top_off",
            "wrap_bottom_line",
            "wrapped_visible",
            "tabs",
            "active",
            "file_meta",
            "disk_changed",
            "recent",
            "show_recent",
            "watch_started",
            "pending_close",
            "close_hooked",
            "allow_window_close",
        ]:
            content = content.replace(f"\n    {field}:", f"\n    pub(crate) {field}:")
        content = content.replace("enum CloseTarget {", "pub(crate) enum CloseTarget {")
        out.write_text(content, encoding="utf-8", newline="\n")
        return

    if path == "ui/widgets.rs":
        content = (
            "//! Shared UI helpers for editor overlays.\n\n"
            + "use super::super::imports::*;\n\n"
            + body
        )
        out.write_text(content, encoding="utf-8", newline="\n")
        return

    if path in ("render.rs", "input_handler.rs"):
        header = {
            "render.rs": "//! Root `Render` implementation for the editor view.\n",
            "input_handler.rs": "//! IME / platform text input routing.\n",
        }[path]
        if path == "render.rs":
            content = (
                header
                + "\nuse super::imports::*;\n\n"
                + slice_lines(3009, 3013)
                + "\n\n"
                + body
            )
        else:
            content = (
                header
                + "\nuse super::imports::*;\n\n"
                + slice_lines(3141, 3279)
                + "\n\nimpl EngineEditor {\n"
                + slice_lines(3282, 3309)
                + "}\n"
            )
        out.write_text(content, encoding="utf-8", newline="\n")
        return

    if path.startswith("ui/"):
        content = (
            f"//! UI fragment: `{path}`.\n\n"
            + "use super::super::imports::*;\n\n"
            + "impl EngineEditor {\n"
            + body
            + "}\n"
        )
        out.write_text(content, encoding="utf-8", newline="\n")
        return

    # impl/*.rs
    content = (
        f"//! `EngineEditor` — `{path.replace('impl/', '').replace('.rs', '')}`.\n\n"
        + "use super::super::imports::*;\n\n"
        + "impl EngineEditor {\n"
        + body
        + "}\n"
    )
    out.write_text(content, encoding="utf-8", newline="\n")


for path, start, end in SECTIONS:
    write_impl(path, start, end)

def write_mod_rs() -> None:
    (EV / "mod.rs").write_text(
        """//! Engine-backed editor view (module root).

mod canvas;
mod editor;
mod imports;
mod language;
mod r#impl;
mod input_handler;
mod render;
mod state;
mod text_util;
mod ui;

pub use editor::EngineEditor;
pub use language::language_for_path;
""",
        encoding="utf-8",
        newline="\n",
    )


# impl/mod.rs
impl_mod = """//! `EngineEditor` behavior split by domain.

mod clipboard;
mod close_confirm;
mod core;
mod disk_watch;
mod editing;
mod file_io;
mod find;
mod goto;
mod keyboard;
mod mouse_scroll;
mod movement;
mod recent;
mod search_panel;
mod selection;
mod tabs;
"""
(EV / "impl/mod.rs").write_text(impl_mod, encoding="utf-8", newline="\n")

# ui/mod.rs
ui_mod = """//! Editor chrome and overlay UI builders.

mod chrome;
mod find_bar;
mod goto_bar;
mod overlays;
mod scrollbars;
mod search_panel_ui;
mod widgets;

pub(crate) use widgets::{bar_button, render_input_field};
"""
(EV / "ui/mod.rs").write_text(ui_mod, encoding="utf-8", newline="\n")

write_mod_rs()
print("Split complete. Run cargo build to fix imports.")
