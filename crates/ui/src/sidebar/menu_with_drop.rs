//! Sidebar menu that wraps nav rows with folder drop targets.
//!
//! Upstream `gpui-component` does not expose file drop on sidebar items; CyberFiles
//! implements that here without patching the dependency. Row height matches the title tab bar.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    div,
    prelude::{FluentBuilder as _, *},
    px, AnyElement, App, ClickEvent, ElementId, IntoElement, MouseButton, SharedString,
    StyleRefinement, Styled, Window,
};
use gpui_component::{
    h_flex, sidebar::SidebarItem, v_flex, ActiveTheme as _, Collapsible, StyledExt,
};

use super::constants::SIDEBAR_ITEM_HEIGHT;
use crate::drag::DraggedFilePaths;
use crate::popup_menu::{ContextMenuExt as _, PopupMenu};
use crate::shell_icon::shell_icon_for_path;

#[derive(Clone)]
struct FolderDropHandlers {
    on_drag_move: Rc<dyn Fn(&mut Window, &mut App)>,
    on_drop: Rc<dyn Fn(&DraggedFilePaths, &mut Window, &mut App)>,
}

#[derive(Clone)]
enum SidebarRowIcon {
    App(Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>),
    Shell(PathBuf),
}

#[derive(Clone)]
enum SidebarRow {
    Item {
        label: SharedString,
        icon: SidebarRowIcon,
        active: bool,
        handler: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>,
        on_middle_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
        context_menu: Option<Rc<dyn Fn(PopupMenu, &mut Window, &mut App) -> PopupMenu>>,
        drop_handlers: Option<FolderDropHandlers>,
    },
}

/// [`gpui_component::sidebar::SidebarMenu`] equivalent with optional per-row file drop.
#[derive(Clone)]
pub struct SidebarMenuWithDrop {
    style: StyleRefinement,
    collapsed: bool,
    rows: Vec<SidebarRow>,
}

fn render_item_row(
    row_id: SharedString,
    label: SharedString,
    icon: SidebarRowIcon,
    active: bool,
    collapsed: bool,
    handler: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>,
    on_middle_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    context_menu: Option<Rc<dyn Fn(PopupMenu, &mut Window, &mut App) -> PopupMenu>>,
    drop_handlers: Option<FolderDropHandlers>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let is_hoverable = !active;
    let icon_element = match icon {
        SidebarRowIcon::App(icon) => icon(window, cx),
        SidebarRowIcon::Shell(path) => {
            shell_icon_for_path(&path, px(16.), window).into_any_element()
        }
    };

    let item_inner = h_flex()
        .id("item")
        .w_full()
        .h(SIDEBAR_ITEM_HEIGHT)
        .px_2()
        .gap_x_2()
        .items_center()
        .rounded(cx.theme().radius)
        .text_sm()
        .when(is_hoverable, |this| {
            this.hover(|this| {
                this.bg(cx.theme().sidebar_accent.opacity(0.8))
                    .text_color(cx.theme().sidebar_accent_foreground)
            })
        })
        .when(active, |this| {
            this.font_medium()
                .bg(cx.theme().sidebar_accent)
                .text_color(cx.theme().sidebar_accent_foreground)
        })
        .when(collapsed, |this| this.justify_center())
        .child(icon_element)
        .when(!collapsed, |this| this.child(label))
        .on_click(move |event, window, cx| handler(event, window, cx));

    let item_any: AnyElement = if let Some(menu) = context_menu {
        item_inner
            .context_menu(move |popup, window, cx| menu(popup, window, cx))
            .into_any_element()
    } else {
        item_inner.into_any_element()
    };

    let mut row_el = div().id(row_id).w_full().child(item_any);
    if let Some(middle) = on_middle_click {
        row_el = row_el.on_mouse_down(MouseButton::Middle, move |_, window, cx| {
            middle(window, cx);
        });
    }
    if let Some(handlers) = drop_handlers {
        let drag_move = handlers.on_drag_move.clone();
        let drop = handlers.on_drop.clone();
        row_el = row_el
            .on_drag_move::<DraggedFilePaths>(move |_, window, cx| {
                drag_move(window, cx);
            })
            .on_drop(move |paths: &DraggedFilePaths, window, cx| {
                drop(paths, window, cx);
            });
    }

    row_el.into_any_element()
}

impl SidebarMenuWithDrop {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            collapsed: false,
            rows: Vec::new(),
        }
    }

    pub fn push_item(
        &mut self,
        label: impl Into<SharedString>,
        icon: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
        active: bool,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        on_middle_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
        context_menu: Option<Rc<dyn Fn(PopupMenu, &mut Window, &mut App) -> PopupMenu>>,
    ) {
        self.rows.push(SidebarRow::Item {
            label: label.into(),
            icon: SidebarRowIcon::App(Rc::new(icon)),
            active,
            handler: Rc::new(handler),
            on_middle_click,
            context_menu,
            drop_handlers: None,
        });
    }

    pub fn push_shell_path(
        &mut self,
        label: impl Into<SharedString>,
        path: PathBuf,
        active: bool,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        on_middle_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
        context_menu: Option<Rc<dyn Fn(PopupMenu, &mut Window, &mut App) -> PopupMenu>>,
    ) {
        self.rows.push(SidebarRow::Item {
            label: label.into(),
            icon: SidebarRowIcon::Shell(path),
            active,
            handler: Rc::new(handler),
            on_middle_click,
            context_menu,
            drop_handlers: None,
        });
    }

    pub fn push_shell_path_with_folder_drop(
        &mut self,
        label: impl Into<SharedString>,
        path: PathBuf,
        active: bool,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        on_middle_click: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
        context_menu: Option<Rc<dyn Fn(PopupMenu, &mut Window, &mut App) -> PopupMenu>>,
        on_drag_move: impl Fn(&mut Window, &mut App) + 'static,
        on_drop: impl Fn(&DraggedFilePaths, &mut Window, &mut App) + 'static,
    ) {
        self.rows.push(SidebarRow::Item {
            label: label.into(),
            icon: SidebarRowIcon::Shell(path),
            active,
            handler: Rc::new(handler),
            on_middle_click,
            context_menu,
            drop_handlers: Some(FolderDropHandlers {
                on_drag_move: Rc::new(on_drag_move),
                on_drop: Rc::new(on_drop),
            }),
        });
    }
}

impl Collapsible for SidebarMenuWithDrop {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl Styled for SidebarMenuWithDrop {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl SidebarItem for SidebarMenuWithDrop {
    fn render(
        self,
        id: impl Into<ElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let id = id.into();
        let collapsed = self.collapsed;

        v_flex()
            .w_full()
            .gap_2()
            .refine_style(&self.style)
            .children(self.rows.into_iter().enumerate().map(|(ix, row)| {
                let row_id = SharedString::from(format!("{}-{}", id, ix));
                match row {
                    SidebarRow::Item {
                        label,
                        icon,
                        active,
                        handler,
                        on_middle_click,
                        context_menu,
                        drop_handlers,
                    } => render_item_row(
                        row_id,
                        label,
                        icon,
                        active,
                        collapsed,
                        handler,
                        on_middle_click,
                        context_menu,
                        drop_handlers,
                        window,
                        cx,
                    ),
                }
            }))
    }
}
