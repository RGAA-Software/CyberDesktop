//! Editor surface context menu (right-click).

use gpui::{App, WeakEntity, Window};

use crate::popup_menu::{PopupMenu, PopupMenuItem};

use super::CyberEditorPage;

pub(crate) fn editor_surface_context_menu(
    menu: PopupMenu,
    page: WeakEntity<CyberEditorPage>,
    _window: &mut Window,
    cx: &mut App,
) -> PopupMenu {
    let has_selection = page
        .read_with(cx, |page, _| page.has_editor_selection())
        .unwrap_or(false);

    let page_undo = page.clone();
    let page_redo = page.clone();
    let page_cut = page.clone();
    let page_copy = page.clone();
    let page_paste = page.clone();
    let page_select_all = page.clone();
    let page_find = page.clone();

    menu.item(PopupMenuItem::new("Undo").on_click(move |_, window, cx| {
        let _ = page_undo.update(cx, |page, cx| page.run_editor_undo(window, cx));
    }))
    .item(PopupMenuItem::new("Redo").on_click(move |_, window, cx| {
        let _ = page_redo.update(cx, |page, cx| page.run_editor_redo(window, cx));
    }))
    .separator()
    .item(
        PopupMenuItem::new("Cut")
            .disabled(!has_selection)
            .on_click(move |_, window, cx| {
                let _ = page_cut.update(cx, |page, cx| page.run_editor_cut(window, cx));
            }),
    )
    .item(
        PopupMenuItem::new("Copy")
            .disabled(!has_selection)
            .on_click(move |_, window, cx| {
                let _ = page_copy.update(cx, |page, cx| page.run_editor_copy(window, cx));
            }),
    )
    .item(PopupMenuItem::new("Paste").on_click(move |_, window, cx| {
        let _ = page_paste.update(cx, |page, cx| page.run_editor_paste(window, cx));
    }))
    .separator()
    .item(PopupMenuItem::new("Select All").on_click(move |_, window, cx| {
        let _ = page_select_all.update(cx, |page, cx| page.run_select_all(window, cx));
    }))
    .separator()
    .item(PopupMenuItem::new("Find...").on_click(move |_, window, cx| {
        let _ = page_find.update(cx, |page, cx| page.open_find_dialog(window, cx));
    }))
}
