//! Editor right-click context menu.

use gpui::{
    Anchor, App, Context, DismissEvent, Entity, MouseDownEvent, Point, Subscription, WeakEntity,
    Window, anchored, deferred, px,
};

use cyber_desktop_ui::{PopupMenu, PopupMenuItem};

use super::super::imports::*;

pub(crate) struct EditorContextMenuState {
    position: Point<Pixels>,
    menu: Entity<PopupMenu>,
    _subscription: Subscription,
}

impl EngineEditor {
    pub(crate) fn on_mouse_right(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        self.input_target = InputTarget::Document;
        let idx = self.index_for_position(event.position);
        let primary = self.document.selections().primary();
        // Preserve the selection when right-clicking inside it (VS Code-style).
        let inside_selection = !primary.is_empty()
            && idx >= primary.start()
            && idx <= primary.end();
        if !inside_selection {
            self.document.set_caret(idx);
        }
        self.open_context_menu(event.position, window, cx);
        cx.stop_propagation();
    }

    fn open_context_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_context_menu();
        self.context_menu_pending = Some(position);
        let editor_handle = cx.weak_entity();
        window.defer(cx, move |window, cx| {
            Self::install_context_menu(&editor_handle, window, cx);
        });
        cx.notify();
    }

    /// Build `PopupMenu` outside any `EngineEditor::update` (avoids `double_lease_panic`).
    fn install_context_menu(
        editor_handle: &WeakEntity<Self>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(editor_entity) = editor_handle.upgrade() else {
            return;
        };
        let position = {
            let editor = editor_entity.read(cx);
            let Some(pos) = editor.context_menu_pending else {
                return;
            };
            pos
        };

        let menu = PopupMenu::build(window, cx, {
            let editor = editor_entity.clone();
            move |menu, _window, cx| build_editor_context_menu(menu, editor, cx)
        });

        let editor_weak = editor_entity.downgrade();
        let subscription = window.subscribe(&menu, cx, {
            let dismiss_weak = editor_weak.clone();
            move |_, _: &DismissEvent, window, cx| {
                let _ = dismiss_weak.update(cx, |this, cx| {
                    this.dismiss_context_menu();
                    cx.notify();
                });
                window.refresh();
            }
        });

        let _ = editor_weak.update(cx, |this, cx| {
            if this.context_menu_pending != Some(position) {
                return;
            }
            this.context_menu = Some(EditorContextMenuState {
                position,
                menu,
                _subscription: subscription,
            });
            cx.notify();
        });
    }

    pub(crate) fn dismiss_context_menu(&mut self) {
        self.context_menu_pending = None;
        self.context_menu = None;
    }

    pub(crate) fn render_context_menu_overlay(&self, window: &Window) -> impl IntoElement {
        let Some(state) = &self.context_menu else {
            return div().into_any_element();
        };
        let position = state.position;
        let menu = state.menu.clone();
        deferred(
            anchored().child(
                div()
                    .w(window.bounds().size.width)
                    .h(window.bounds().size.height)
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                    .child(
                        anchored()
                            .position(position)
                            .snap_to_window_with_margin(px(8.))
                            .anchor(Anchor::TopLeft)
                            .child(menu),
                    ),
            ),
        )
        .with_priority(1)
        .into_any_element()
    }
}

fn build_editor_context_menu(
    mut menu: PopupMenu,
    editor: Entity<EngineEditor>,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let snapshot = editor.read(cx);
    let has_selection = !snapshot.document.selections().primary().is_empty();
    let can_undo = snapshot.document.can_undo();
    let can_redo = snapshot.document.can_redo();
    let can_paste = cx
        .read_from_clipboard()
        .and_then(|item| item.text())
        .is_some();
    let caret_line = snapshot
        .document
        .buffer()
        .char_to_position(snapshot.document.selections().primary().head)
        .line;
    let can_fold = snapshot.crease_at(caret_line).is_some()
        || snapshot.is_folded_header(caret_line);
    let has_folds = !snapshot.active_folds.is_empty();

    menu = menu
        .item(
            PopupMenuItem::new(t!("editor.menu.undo"))
                .disabled(!can_undo)
                .on_click({
                    let editor = editor.clone();
                    move |_, _, cx| {
                        editor.update(cx, |this, cx| {
                            this.document.undo();
                            this.changed(cx);
                        });
                    }
                }),
        )
        .item(
            PopupMenuItem::new(t!("editor.menu.redo"))
                .disabled(!can_redo)
                .on_click({
                    let editor = editor.clone();
                    move |_, _, cx| {
                        editor.update(cx, |this, cx| {
                            this.document.redo();
                            this.changed(cx);
                        });
                    }
                }),
        )
        .separator()
        .item(
            PopupMenuItem::new(t!("editor.menu.cut"))
                .disabled(!has_selection)
                .on_click({
                    let editor = editor.clone();
                    move |_, _, cx| {
                        editor.update(cx, |this, cx| {
                            this.cut(cx);
                        });
                    }
                }),
        )
        .item(
            PopupMenuItem::new(t!("editor.menu.copy"))
                .disabled(!has_selection)
                .on_click({
                    let editor = editor.clone();
                    move |_, _, cx| {
                        editor.update(cx, |this, cx| {
                            this.copy(cx);
                        });
                    }
                }),
        )
        .item(
            PopupMenuItem::new(t!("editor.menu.paste"))
                .disabled(!can_paste)
                .on_click({
                    let editor = editor.clone();
                    move |_, _, cx| {
                        editor.update(cx, |this, cx| {
                            this.paste(cx);
                        });
                    }
                }),
        )
        .separator()
        .item(
            PopupMenuItem::new(t!("editor.menu.select_all")).on_click({
                let editor = editor.clone();
                move |_, _, cx| {
                    editor.update(cx, |this, cx| {
                        this.document.select_all();
                        this.changed(cx);
                    });
                }
            }),
        )
        .item(
            PopupMenuItem::new(t!("editor.menu.select_line")).on_click({
                let editor = editor.clone();
                move |_, _, cx| {
                    editor.update(cx, |this, cx| {
                        this.select_line(cx);
                    });
                }
            }),
        )
        .separator()
        .item(
            PopupMenuItem::new(t!("editor.menu.find")).on_click({
                let editor = editor.clone();
                move |_, window, cx| {
                    editor.update(cx, |this, cx| this.open_find(false, window, cx));
                }
            }),
        )
        .item(
            PopupMenuItem::new(t!("editor.menu.go_to_line")).on_click({
                let editor = editor.clone();
                move |_, window, cx| {
                    editor.update(cx, |this, cx| this.open_goto(window, cx));
                }
            }),
        )
        .item(
            PopupMenuItem::new(t!("editor.menu.toggle_comment")).on_click({
                let editor = editor.clone();
                move |_, _, cx| {
                    editor.update(cx, |this, cx| {
                        this.toggle_comment(cx);
                    });
                }
            }),
        )
        .separator()
        .item(
            PopupMenuItem::new(t!("editor.menu.toggle_fold"))
                .disabled(!can_fold)
                .on_click({
                    let editor = editor.clone();
                    move |_, _, cx| {
                        editor.update(cx, |this, cx| this.toggle_fold_at_caret(cx));
                    }
                }),
        )
        .item(
            PopupMenuItem::new(t!("editor.menu.fold_all")).on_click({
                let editor = editor.clone();
                move |_, _, cx| {
                    editor.update(cx, |this, cx| {
                        this.fold_all(cx);
                    });
                }
            }),
        )
        .item(
            PopupMenuItem::new(t!("editor.menu.unfold_all"))
                .disabled(!has_folds)
                .on_click({
                    let editor = editor.clone();
                    move |_, _, cx| {
                        editor.update(cx, |this, cx| {
                            this.unfold_all(cx);
                        });
                    }
                }),
        );

    menu
}
