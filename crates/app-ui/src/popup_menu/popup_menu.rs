//! Fork of gpui-component `PopupMenu` (MIT) — maintained in CyberFiles for row layout and Shell PNG icons.
//!
//! Upstream: <https://github.com/longbridge/gpui-component> `crates/files-app-ui/src/menu/popup_menu.rs`

use super::actions::{Cancel, Confirm, SelectDown, SelectLeft, SelectRight, SelectUp};
use super::menu_item::MenuItemElement;
use super::menu_scrollbar::{MenuScrollbar, MENU_SCROLLBAR_GAP, MENU_SCROLLBAR_WIDTH};
use gpui::{
    anchored, deferred, div, img, prelude::FluentBuilder, px, Action, Anchor, AnyElement, App,
    AppContext, Bounds, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    Image, ImageFormat, InteractiveElement, IntoElement, KeyBinding, ObjectFit, ParentElement,
    Pixels, Render, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, StyledImage,
    WeakEntity, Window,
};
use gpui::{ClickEvent, Half, MouseDownEvent, Point, Subscription};
use gpui_component::{
    h_flex, kbd::Kbd, scroll::ScrollbarShow, v_flex, ActiveTheme, ElementExt, Icon, IconName, Side,
    Sizable as _, Size, StyledExt,
};

use std::rc::Rc;
use std::sync::Arc;

/// Key context for CyberFiles popup menus (separate from gpui-component `PopupMenu`).
const CONTEXT: &str = "CyberDesktopPopupMenu";

/// Default row height for menu and submenu rows.
pub const DEFAULT_ITEM_ROW_HEIGHT: Pixels = px(34.);
/// Default minimum width for popup and context menus.
pub const DEFAULT_POPUP_MENU_MIN_WIDTH: Pixels = px(225.);
const MENU_CONTAINER_RADIUS: Pixels = px(14.);
const MENU_ITEM_RADIUS: Pixels = px(9.);
const MENU_EDGE_PADDING: Pixels = px(6.);

/// Fixed width/height for the left icon gutter (Material + Shell PNG).
pub const ICON_SLOT_SIZE: Pixels = px(16.);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", Confirm { secondary: false }, Some(CONTEXT)),
        KeyBinding::new("escape", Cancel, Some(CONTEXT)),
        KeyBinding::new("up", SelectUp, Some(CONTEXT)),
        KeyBinding::new("down", SelectDown, Some(CONTEXT)),
        KeyBinding::new("left", SelectLeft, Some(CONTEXT)),
        KeyBinding::new("right", SelectRight, Some(CONTEXT)),
    ]);
}

/// An menu item in a popup menu.
pub enum PopupMenuItem {
    /// A menu separator item.
    Separator,
    /// A non-interactive label item.
    Label(SharedString),
    /// A standard menu item.
    Item {
        icon: Option<Icon>,
        icon_element: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>>,
        /// Full-color Shell (or other) bitmap; rendered with GPUI `img`, not [`Icon`] tinting.
        icon_png: Option<Arc<Vec<u8>>>,
        label: SharedString,
        disabled: bool,
        checked: bool,
        is_link: bool,
        action: Option<Box<dyn Action>>,
        // For link item
        handler: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    },
    /// A menu item with custom element render.
    ElementItem {
        icon: Option<Icon>,
        icon_element: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>>,
        disabled: bool,
        checked: bool,
        action: Option<Box<dyn Action>>,
        render: Box<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>,
        handler: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    },
    /// A submenu item that opens another popup menu.
    ///
    /// NOTE: This is only supported when the parent menu is not `scrollable`.
    Submenu {
        icon: Option<Icon>,
        icon_element: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>>,
        icon_png: Option<Arc<Vec<u8>>>,
        label: SharedString,
        disabled: bool,
        menu: Entity<PopupMenu>,
    },
}

impl FluentBuilder for PopupMenuItem {}
impl PopupMenuItem {
    /// Create a new menu item with the given label.
    #[inline]
    pub fn new(label: impl Into<SharedString>) -> Self {
        PopupMenuItem::Item {
            icon: None,
            icon_element: None,
            icon_png: None,
            label: label.into(),
            disabled: false,
            checked: false,
            action: None,
            is_link: false,
            handler: None,
        }
    }

    /// Create a new menu item with custom element render.
    #[inline]
    pub fn element<F, E>(builder: F) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        PopupMenuItem::ElementItem {
            icon: None,
            icon_element: None,
            disabled: false,
            checked: false,
            action: None,
            render: Box::new(move |window, cx| builder(window, cx).into_any_element()),
            handler: None,
        }
    }

    /// Create a new submenu item that opens another popup menu.
    #[inline]
    pub fn submenu(label: impl Into<SharedString>, menu: Entity<PopupMenu>) -> Self {
        PopupMenuItem::Submenu {
            icon: None,
            icon_element: None,
            icon_png: None,
            label: label.into(),
            disabled: false,
            menu,
        }
    }

    /// Create a separator menu item.
    #[inline]
    pub fn separator() -> Self {
        PopupMenuItem::Separator
    }

    /// Creates a label menu item.
    #[inline]
    pub fn label(label: impl Into<SharedString>) -> Self {
        PopupMenuItem::Label(label.into())
    }

    /// Set the icon for the menu item.
    ///
    /// Only works for [`PopupMenuItem::Item`], [`PopupMenuItem::ElementItem`] and [`PopupMenuItem::Submenu`].
    /// Set a full-color PNG for the left icon slot (Windows Shell menu icons).
    pub fn icon_png(mut self, png: Arc<Vec<u8>>) -> Self {
        match &mut self {
            PopupMenuItem::Item {
                icon_png: p,
                icon: i,
                icon_element: e,
                ..
            }
            | PopupMenuItem::Submenu {
                icon_png: p,
                icon: i,
                icon_element: e,
                ..
            } => {
                *p = Some(png);
                *i = None;
                *e = None;
            }
            _ => {}
        }
        self
    }

    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        match &mut self {
            PopupMenuItem::Item {
                icon: i,
                icon_element: e,
                icon_png: p,
                ..
            } => {
                *i = Some(icon.into());
                *e = None;
                *p = None;
            }
            PopupMenuItem::ElementItem {
                icon: i,
                icon_element: e,
                ..
            } => {
                *i = Some(icon.into());
                *e = None;
            }
            PopupMenuItem::Submenu {
                icon: i,
                icon_element: e,
                icon_png: p,
                ..
            } => {
                *i = Some(icon.into());
                *e = None;
                *p = None;
            }
            _ => {}
        }
        self
    }

    pub fn icon_element(
        mut self,
        builder: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        let builder = Rc::new(builder);
        match &mut self {
            PopupMenuItem::Item {
                icon,
                icon_element,
                icon_png,
                ..
            } => {
                *icon = None;
                *icon_png = None;
                *icon_element = Some(builder);
            }
            PopupMenuItem::ElementItem {
                icon, icon_element, ..
            } => {
                *icon = None;
                *icon_element = Some(builder);
            }
            PopupMenuItem::Submenu {
                icon,
                icon_element,
                icon_png,
                ..
            } => {
                *icon = None;
                *icon_png = None;
                *icon_element = Some(builder);
            }
            _ => {}
        }
        self
    }

    /// Set the action for the menu item.
    ///
    /// Only works for [`PopupMenuItem::Item`] and [`PopupMenuItem::ElementItem`].
    pub fn action(mut self, action: Box<dyn Action>) -> Self {
        match &mut self {
            PopupMenuItem::Item { action: a, .. } => {
                *a = Some(action);
            }
            PopupMenuItem::ElementItem { action: a, .. } => {
                *a = Some(action);
            }
            _ => {}
        }
        self
    }

    /// Set the disabled state for the menu item.
    ///
    /// Only works for [`PopupMenuItem::Item`], [`PopupMenuItem::ElementItem`] and [`PopupMenuItem::Submenu`].
    pub fn disabled(mut self, disabled: bool) -> Self {
        match &mut self {
            PopupMenuItem::Item { disabled: d, .. } => {
                *d = disabled;
            }
            PopupMenuItem::ElementItem { disabled: d, .. } => {
                *d = disabled;
            }
            PopupMenuItem::Submenu { disabled: d, .. } => {
                *d = disabled;
            }
            _ => {}
        }
        self
    }

    /// Set checked state for the menu item.
    ///
    /// NOTE: If `check_side` is [`Side::Left`], the icon will replace with a check icon.
    pub fn checked(mut self, checked: bool) -> Self {
        match &mut self {
            PopupMenuItem::Item { checked: c, .. } => {
                *c = checked;
            }
            PopupMenuItem::ElementItem { checked: c, .. } => {
                *c = checked;
            }
            _ => {}
        }
        self
    }

    /// Add a click handler for the menu item.
    ///
    /// Only works for [`PopupMenuItem::Item`] and [`PopupMenuItem::ElementItem`].
    pub fn on_click<F>(mut self, handler: F) -> Self
    where
        F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    {
        match &mut self {
            PopupMenuItem::Item { handler: h, .. } => {
                *h = Some(Rc::new(handler));
            }
            PopupMenuItem::ElementItem { handler: h, .. } => {
                *h = Some(Rc::new(handler));
            }
            _ => {}
        }
        self
    }

    /// Create a link menu item.
    #[inline]
    pub fn link(label: impl Into<SharedString>, href: impl Into<String>) -> Self {
        let href = href.into();
        PopupMenuItem::Item {
            icon: None,
            icon_element: None,
            icon_png: None,
            label: label.into(),
            disabled: false,
            checked: false,
            action: None,
            is_link: true,
            handler: Some(Rc::new(move |_, _, cx| cx.open_url(&href))),
        }
    }

    #[inline]
    fn is_clickable(&self) -> bool {
        !matches!(self, PopupMenuItem::Separator)
            && matches!(
                self,
                PopupMenuItem::Item {
                    disabled: false,
                    ..
                } | PopupMenuItem::ElementItem {
                    disabled: false,
                    ..
                } | PopupMenuItem::Submenu {
                    disabled: false,
                    ..
                }
            )
    }

    #[inline]
    fn is_separator(&self) -> bool {
        matches!(self, PopupMenuItem::Separator)
    }

    fn has_left_icon(&self, check_side: Side) -> bool {
        match self {
            PopupMenuItem::Item {
                icon,
                icon_element,
                icon_png,
                checked,
                ..
            } => {
                icon.is_some()
                    || icon_element.is_some()
                    || icon_png.is_some()
                    || (check_side.is_left() && *checked)
            }
            PopupMenuItem::ElementItem {
                icon,
                icon_element,
                checked,
                ..
            } => icon.is_some() || icon_element.is_some() || (check_side.is_left() && *checked),
            PopupMenuItem::Submenu {
                icon,
                icon_element,
                icon_png,
                ..
            } => {
                icon.is_some() || icon_element.is_some() || icon_png.is_some()
            }
            _ => false,
        }
    }

    #[inline]
    fn is_checked(&self) -> bool {
        match self {
            PopupMenuItem::Item { checked, .. } => *checked,
            PopupMenuItem::ElementItem { checked, .. } => *checked,
            _ => false,
        }
    }
}

pub struct PopupMenu {
    pub(crate) focus_handle: FocusHandle,
    pub(crate) menu_items: Vec<PopupMenuItem>,
    /// The focus handle of Entity to handle actions.
    pub(crate) action_context: Option<FocusHandle>,
    selected_index: Option<usize>,
    min_width: Option<Pixels>,
    max_width: Option<Pixels>,
    max_height: Option<Pixels>,
    bounds: Bounds<Pixels>,
    size: Size,
    check_side: Side,

    /// The parent menu of this menu, if this is a submenu
    parent_menu: Option<WeakEntity<Self>>,
    scrollable: bool,
    /// When `scrollable` and the menu actually overflows, keep the vertical
    /// scrollbar permanently visible instead of fading it out after idle.
    ///
    /// Has no effect when the content fits (no scrollbar is rendered at all).
    scrollbar_always: bool,
    external_link_icon: bool,
    scroll_handle: ScrollHandle,
    // This will update on render
    submenu_anchor: (Anchor, Pixels),
    /// Row height for items and submenu headers (default [`DEFAULT_ITEM_ROW_HEIGHT`]).
    item_row_height: Option<Pixels>,

    _subscriptions: Vec<Subscription>,
}

impl PopupMenu {
    pub(crate) fn new(cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            action_context: None,
            parent_menu: None,
            menu_items: Vec::new(),
            selected_index: None,
            min_width: None,
            max_width: None,
            max_height: None,
            check_side: Side::Left,
            bounds: Bounds::default(),
            scrollable: false,
            scrollbar_always: false,
            scroll_handle: ScrollHandle::default(),
            external_link_icon: true,
            size: Size::default(),
            submenu_anchor: (Anchor::TopLeft, Pixels::ZERO),
            item_row_height: None,
            _subscriptions: vec![],
        }
    }

    fn item_row_height(&self) -> Pixels {
        self.item_row_height.unwrap_or(DEFAULT_ITEM_ROW_HEIGHT)
    }

    /// Approximate the natural (unscrolled) pixel height of the items area.
    ///
    /// This deliberately mirrors the layout produced by [`Self::render`] (row
    /// heights from [`Self::item_row_height`], `gap_y_0p5` between rows and the
    /// `p_1` container padding) so we can decide *before* layout whether a
    /// `scrollable` menu will actually overflow its `max_h`.
    ///
    /// Why estimate instead of trusting the layout/scrollbar at runtime: a
    /// submenu flyout is an absolutely-positioned, `deferred` element, and its
    /// scroll viewport can be measured slightly shorter than its content even
    /// when everything visually fits. Combined with `flex_shrink_0` rows (which
    /// can no longer shrink to hide that mismatch) this produced a spurious
    /// scrollbar on small nested submenus. Gating the scroll machinery on this
    /// estimate keeps small menus completely plain (no viewport, no scrollbar,
    /// no clipping) and only engages scrolling for genuinely tall menus.
    ///
    /// The numbers are intentionally coarse — menus are either clearly small or
    /// clearly tall relative to 4/5 of the window, so a few px of error never
    /// flips the decision in practice.
    fn estimated_content_height(&self) -> Pixels {
        // Separator row: 2px bottom border + ~4px vertical margin (`my_0p5`).
        const SEPARATOR_HEIGHT: Pixels = px(9.);
        // Vertical gap inserted between every row (`gap_y_0p5`).
        const ROW_GAP: Pixels = px(2.);
        // Items container top + bottom padding (`p(MENU_EDGE_PADDING)` on both
        // sides; MENU_EDGE_PADDING is 6px). The old estimate used 8px which
        // under-counted and could skip scrolling when content barely overflows.
        const VERTICAL_PADDING: Pixels = px(12.);

        // Must match the per-row height chosen in `render_item`.
        let item_height = match self.size {
            Size::Small => px(24.),
            _ => self.item_row_height(),
        };

        let count = self.menu_items.len();
        let mut total = VERTICAL_PADDING;
        for item in &self.menu_items {
            total += if item.is_separator() {
                SEPARATOR_HEIGHT
            } else {
                item_height
            };
        }
        // `gap_y_0p5` only sits *between* rows, so there are `count - 1` gaps.
        if count > 1 {
            total += ROW_GAP * (count as f32 - 1.0);
        }
        total
    }

    /// Set uniform row height for normal items and submenu rows (default 32px).
    pub fn item_row_h(mut self, height: impl Into<Pixels>) -> Self {
        self.item_row_height = Some(height.into());
        self
    }

    pub fn build(
        window: &mut Window,
        cx: &mut App,
        f: impl FnOnce(Self, &mut Window, &mut Context<PopupMenu>) -> Self,
    ) -> Entity<Self> {
        cx.new(|cx| f(Self::new(cx), window, cx))
    }

    /// Set the focus handle of Entity to handle actions.
    ///
    /// When the menu is dismissed or before an action is triggered, the focus will be returned to this handle.
    ///
    /// Then the action will be dispatched to this handle.
    pub fn action_context(mut self, handle: FocusHandle) -> Self {
        self.action_context = Some(handle);
        self
    }

    /// Set min width of the popup menu, default is [`DEFAULT_POPUP_MENU_MIN_WIDTH`].
    pub fn min_w(mut self, width: impl Into<Pixels>) -> Self {
        self.min_width = Some(width.into());
        self
    }

    /// Set max width of the popup menu, default is 500px
    pub fn max_w(mut self, width: impl Into<Pixels>) -> Self {
        self.max_width = Some(width.into());
        self
    }

    /// Set max height of the popup menu, default is half of the window height
    pub fn max_h(mut self, height: impl Into<Pixels>) -> Self {
        self.max_height = Some(height.into());
        self
    }

    /// Allow the menu to scroll vertically once its content exceeds `max_h`.
    ///
    /// This is only an *opt-in*: the scroll viewport and scrollbar are engaged
    /// lazily at render time (see `needs_scroll` in [`Self::render`]) and only
    /// when [`Self::estimated_content_height`] is taller than the resolved
    /// `max_h`. A scrollable menu whose items fit renders exactly like a normal
    /// menu, with no scrollbar and no clipping.
    ///
    /// Unlike the upstream gpui-component menu, sub-menus keep working while
    /// scrollable: each submenu flyout is wrapped in [`deferred`] (see
    /// `render_item`) so it paints at the top level, escaping this menu's
    /// `overflow_y_scroll` clip mask.
    pub fn scrollable(mut self, scrollable: bool) -> Self {
        self.scrollable = scrollable;
        self
    }

    /// Keep the vertical scrollbar permanently visible (no idle fade-out) while
    /// the menu is actually scrolling.
    ///
    /// Only meaningful together with [`Self::scrollable`], and only takes effect
    /// when the content overflows — a menu that fits never shows a scrollbar.
    pub fn scrollbar_always(mut self, always: bool) -> Self {
        self.scrollbar_always = always;
        self
    }

    /// Set the side to show check icon, default is `Side::Left`.
    pub fn check_side(mut self, side: Side) -> Self {
        self.check_side = side;
        self
    }

    /// Set the menu to show external link icon, default is true.
    pub fn external_link_icon(mut self, visible: bool) -> Self {
        self.external_link_icon = visible;
        self
    }

    /// Add Menu Item
    pub fn menu(self, label: impl Into<SharedString>, action: Box<dyn Action>) -> Self {
        self.menu_with_disabled(label, action, false)
    }

    /// Add Menu Item with enable state
    pub fn menu_with_enable(
        mut self,
        label: impl Into<SharedString>,
        action: Box<dyn Action>,
        enable: bool,
    ) -> Self {
        self.add_menu_item(label, None, action, !enable, false);
        self
    }

    /// Add Menu Item with disabled state
    pub fn menu_with_disabled(
        mut self,
        label: impl Into<SharedString>,
        action: Box<dyn Action>,
        disabled: bool,
    ) -> Self {
        self.add_menu_item(label, None, action, disabled, false);
        self
    }

    /// Add label
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.menu_items.push(PopupMenuItem::label(label.into()));
        self
    }

    /// Add Menu to open link
    pub fn link(self, label: impl Into<SharedString>, href: impl Into<String>) -> Self {
        self.link_with_disabled(label, href, false)
    }

    /// Add Menu to open link with disabled state
    pub fn link_with_disabled(
        mut self,
        label: impl Into<SharedString>,
        href: impl Into<String>,
        disabled: bool,
    ) -> Self {
        let href = href.into();
        self.menu_items
            .push(PopupMenuItem::link(label, href).disabled(disabled));
        self
    }

    /// Add Menu to open link
    pub fn link_with_icon(
        self,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
        href: impl Into<String>,
    ) -> Self {
        self.link_with_icon_and_disabled(label, icon, href, false)
    }

    /// Add Menu to open link with icon and disabled state
    fn link_with_icon_and_disabled(
        mut self,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
        href: impl Into<String>,
        disabled: bool,
    ) -> Self {
        let href = href.into();
        self.menu_items.push(
            PopupMenuItem::link(label, href)
                .icon(icon)
                .disabled(disabled),
        );
        self
    }

    /// Add Menu Item with Icon.
    pub fn menu_with_icon(
        self,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
        action: Box<dyn Action>,
    ) -> Self {
        self.menu_with_icon_and_disabled(label, icon, action, false)
    }

    /// Add Menu Item with Icon and disabled state
    pub fn menu_with_icon_and_disabled(
        mut self,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
        action: Box<dyn Action>,
        disabled: bool,
    ) -> Self {
        self.add_menu_item(label, Some(icon.into()), action, disabled, false);
        self
    }

    /// Add Menu Item with check icon
    pub fn menu_with_check(
        self,
        label: impl Into<SharedString>,
        checked: bool,
        action: Box<dyn Action>,
    ) -> Self {
        self.menu_with_check_and_disabled(label, checked, action, false)
    }

    /// Add Menu Item with check icon and disabled state
    pub fn menu_with_check_and_disabled(
        mut self,
        label: impl Into<SharedString>,
        checked: bool,
        action: Box<dyn Action>,
        disabled: bool,
    ) -> Self {
        self.add_menu_item(label, None, action, disabled, checked);
        self
    }

    pub fn menu_with_check_icon(
        self,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
        checked: bool,
        action: Box<dyn Action>,
    ) -> Self {
        self.menu_with_check_icon_and_disabled(label, icon, checked, action, false)
    }

    pub fn menu_with_check_icon_and_disabled(
        mut self,
        label: impl Into<SharedString>,
        icon: impl Into<Icon>,
        checked: bool,
        action: Box<dyn Action>,
        disabled: bool,
    ) -> Self {
        self.add_menu_item(label, Some(icon.into()), action, disabled, checked);
        self
    }

    /// Add Menu Item with custom element render.
    pub fn menu_element<F, E>(self, action: Box<dyn Action>, builder: F) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.menu_element_with_check(false, action, builder)
    }

    /// Add Menu Item with custom element render with disabled state.
    pub fn menu_element_with_disabled<F, E>(
        self,
        action: Box<dyn Action>,
        disabled: bool,
        builder: F,
    ) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.menu_element_with_check_and_disabled(false, action, disabled, builder)
    }

    /// Add Menu Item with custom element render with icon.
    pub fn menu_element_with_icon<F, E>(
        self,
        icon: impl Into<Icon>,
        action: Box<dyn Action>,
        builder: F,
    ) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.menu_element_with_icon_and_disabled(icon, action, false, builder)
    }

    /// Add Menu Item with custom element render with check state
    pub fn menu_element_with_check<F, E>(
        self,
        checked: bool,
        action: Box<dyn Action>,
        builder: F,
    ) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.menu_element_with_check_and_disabled(checked, action, false, builder)
    }

    /// Add Menu Item with custom element render with icon and disabled state
    fn menu_element_with_icon_and_disabled<F, E>(
        mut self,
        icon: impl Into<Icon>,
        action: Box<dyn Action>,
        disabled: bool,
        builder: F,
    ) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.menu_items.push(
            PopupMenuItem::element(builder)
                .action(action)
                .icon(icon)
                .disabled(disabled),
        );
        self
    }

    /// Add Menu Item with custom element render with check state and disabled state
    fn menu_element_with_check_and_disabled<F, E>(
        mut self,
        checked: bool,
        action: Box<dyn Action>,
        disabled: bool,
        builder: F,
    ) -> Self
    where
        F: Fn(&mut Window, &mut App) -> E + 'static,
        E: IntoElement,
    {
        self.menu_items.push(
            PopupMenuItem::element(builder)
                .action(action)
                .checked(checked)
                .disabled(disabled),
        );
        self
    }

    /// Add a separator Menu Item
    pub fn separator(mut self) -> Self {
        if self.menu_items.is_empty() {
            return self;
        }

        if let Some(PopupMenuItem::Separator) = self.menu_items.last() {
            return self;
        }

        self.menu_items.push(PopupMenuItem::separator());
        self
    }

    /// Add a Submenu
    pub fn submenu(
        self,
        label: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        self.submenu_with_icon(None, label, window, cx, f)
    }

    /// Add a submenu row with an optional Shell PNG icon (left slot).
    pub fn submenu_with_icon_png(
        mut self,
        label: impl Into<SharedString>,
        icon_png: Option<Arc<Vec<u8>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        let submenu = PopupMenu::build(window, cx, f);
        let parent_menu = cx.entity().downgrade();
        let item_row_height = self.item_row_height;
        submenu.update(cx, |view, _| {
            view.parent_menu = Some(parent_menu);
            view.item_row_height = item_row_height;
        });
        let mut item = PopupMenuItem::submenu(label, submenu);
        if let Some(png) = icon_png {
            item = item.icon_png(png);
        }
        self.menu_items.push(item);
        self
    }

    /// Add a Submenu item with icon
    pub fn submenu_with_icon(
        mut self,
        icon: Option<Icon>,
        label: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        let submenu = PopupMenu::build(window, cx, f);
        let parent_menu = cx.entity().downgrade();
        let item_row_height = self.item_row_height;
        submenu.update(cx, |view, _| {
            view.parent_menu = Some(parent_menu);
            view.item_row_height = item_row_height;
        });

        self.menu_items.push(
            PopupMenuItem::submenu(label, submenu).when_some(icon, |this, icon| this.icon(icon)),
        );
        self
    }

    /// Add a Submenu item with icon and optional disabled state.
    pub fn submenu_with_icon_and_disabled(
        mut self,
        icon: Option<Icon>,
        label: impl Into<SharedString>,
        disabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        let submenu = PopupMenu::build(window, cx, f);
        let parent_menu = cx.entity().downgrade();
        let item_row_height = self.item_row_height;
        submenu.update(cx, |view, _| {
            view.parent_menu = Some(parent_menu);
            view.item_row_height = item_row_height;
        });

        self.menu_items.push(
            PopupMenuItem::submenu(label, submenu)
                .when_some(icon, |this, icon| this.icon(icon))
                .disabled(disabled),
        );
        self
    }

    pub fn submenu_with_element(
        mut self,
        label: impl Into<SharedString>,
        icon_element: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> Self {
        let submenu = PopupMenu::build(window, cx, f);
        let parent_menu = cx.entity().downgrade();
        let item_row_height = self.item_row_height;
        submenu.update(cx, |view, _| {
            view.parent_menu = Some(parent_menu);
            view.item_row_height = item_row_height;
        });

        self.menu_items
            .push(PopupMenuItem::submenu(label, submenu).icon_element(icon_element));
        self
    }

    /// Add menu item.
    pub fn item(mut self, item: impl Into<PopupMenuItem>) -> Self {
        let item: PopupMenuItem = item.into();
        self.menu_items.push(item);
        self
    }

    fn add_menu_item(
        &mut self,
        label: impl Into<SharedString>,
        icon: Option<Icon>,
        action: Box<dyn Action>,
        disabled: bool,
        checked: bool,
    ) -> &mut Self {
        self.menu_items.push(
            PopupMenuItem::new(label)
                .when_some(icon, |item, icon| item.icon(icon))
                .disabled(disabled)
                .checked(checked)
                .action(action),
        );
        self
    }

    pub(crate) fn active_submenu(&self) -> Option<Entity<PopupMenu>> {
        if let Some(ix) = self.selected_index {
            if let Some(item) = self.menu_items.get(ix) {
                return match item {
                    PopupMenuItem::Submenu { menu, .. } => Some(menu.clone()),
                    _ => None,
                };
            }
        }

        None
    }

    pub fn is_empty(&self) -> bool {
        self.menu_items.is_empty()
    }

    fn clickable_menu_items(&self) -> impl Iterator<Item = (usize, &PopupMenuItem)> {
        self.menu_items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.is_clickable())
    }

    fn on_click(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        window.prevent_default();
        self.selected_index = Some(ix);
        self.confirm(&Confirm { secondary: false }, window, cx);
    }

    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        match self.selected_index {
            Some(index) => {
                let item = self.menu_items.get(index);
                match item {
                    Some(PopupMenuItem::Item {
                        handler, action, ..
                    }) => {
                        if let Some(handler) = handler {
                            handler(&ClickEvent::default(), window, cx);
                        } else if let Some(action) = action.as_ref() {
                            self.dispatch_confirm_action(action, window, cx);
                        }

                        self.dismiss(&Cancel, window, cx)
                    }
                    Some(PopupMenuItem::ElementItem {
                        handler, action, ..
                    }) => {
                        if let Some(handler) = handler {
                            handler(&ClickEvent::default(), window, cx);
                        } else if let Some(action) = action.as_ref() {
                            self.dispatch_confirm_action(action, window, cx);
                        }
                        self.dismiss(&Cancel, window, cx)
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn dispatch_confirm_action(
        &self,
        action: &Box<dyn Action>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(context) = self.action_context.as_ref() {
            context.focus(window, cx);
        }

        window.dispatch_action(action.boxed_clone(), cx);
    }

    fn set_selected_index(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.selected_index != Some(ix) {
            self.selected_index = Some(ix);
            self.scroll_handle.scroll_to_item(ix);
            cx.notify();
        }
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let ix = self.selected_index.unwrap_or(0);

        if let Some((prev_ix, _)) = self
            .menu_items
            .iter()
            .enumerate()
            .rev()
            .find(|(i, item)| *i < ix && item.is_clickable())
        {
            self.set_selected_index(prev_ix, cx);
            return;
        }

        let last_clickable_ix = self.clickable_menu_items().last().map(|(ix, _)| ix);
        self.set_selected_index(last_clickable_ix.unwrap_or(0), cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let Some(ix) = self.selected_index else {
            self.set_selected_index(0, cx);
            return;
        };

        if let Some((next_ix, _)) = self
            .menu_items
            .iter()
            .enumerate()
            .find(|(i, item)| *i > ix && item.is_clickable())
        {
            self.set_selected_index(next_ix, cx);
            return;
        }

        self.set_selected_index(0, cx);
    }

    fn select_left(&mut self, _: &SelectLeft, window: &mut Window, cx: &mut Context<Self>) {
        let handled = if matches!(self.submenu_anchor.0, Anchor::TopLeft | Anchor::BottomLeft) {
            self._unselect_submenu(window, cx)
        } else {
            self._select_submenu(window, cx)
        };

        if self.parent_side(cx).is_left() {
            self._focus_parent_menu(window, cx);
        }

        if handled {
            return;
        }

        // For parent AppMenuBar to handle.
        if self.parent_menu.is_none() {
            cx.propagate();
        }
    }

    fn select_right(&mut self, _: &SelectRight, window: &mut Window, cx: &mut Context<Self>) {
        let handled = if matches!(self.submenu_anchor.0, Anchor::TopLeft | Anchor::BottomLeft) {
            self._select_submenu(window, cx)
        } else {
            self._unselect_submenu(window, cx)
        };

        if self.parent_side(cx).is_right() {
            self._focus_parent_menu(window, cx);
        }

        if handled {
            return;
        }

        // For parent AppMenuBar to handle.
        if self.parent_menu.is_none() {
            cx.propagate();
        }
    }

    fn _select_submenu(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(active_submenu) = self.active_submenu() {
            // Focus the submenu, so that can be handle the action.
            active_submenu.update(cx, |view, cx| {
                view.set_selected_index(0, cx);
                view.focus_handle.focus(window, cx);
            });
            cx.notify();
            return true;
        }

        return false;
    }

    fn _unselect_submenu(&mut self, _: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some(active_submenu) = self.active_submenu() {
            active_submenu.update(cx, |view, cx| {
                view.selected_index = None;
                cx.notify();
            });
            return true;
        }

        return false;
    }

    fn _focus_parent_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(parent) = self.parent_menu.as_ref() else {
            return;
        };
        let Some(parent) = parent.upgrade() else {
            return;
        };

        self.selected_index = None;
        parent.update(cx, |view, cx| {
            view.focus_handle.focus(window, cx);
            cx.notify();
        });
    }

    fn parent_side(&self, cx: &App) -> Side {
        let Some(parent) = self.parent_menu.as_ref() else {
            return Side::Left;
        };

        let Some(parent) = parent.upgrade() else {
            return Side::Left;
        };

        match parent.read(cx).submenu_anchor.0 {
            Anchor::TopLeft | Anchor::BottomLeft => Side::Left,
            Anchor::TopRight | Anchor::BottomRight => Side::Right,
            // Center anchors are not used for submenu positioning, but we must cover them.
            _ => Side::Left,
        }
    }

    fn dismiss(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_submenu().is_some() {
            return;
        }

        cx.emit(DismissEvent);

        // Focus back to the previous focused handle.
        if let Some(action_context) = self.action_context.as_ref() {
            window.focus(action_context, cx);
        }

        let Some(parent_menu) = self.parent_menu.clone() else {
            return;
        };

        // Dismiss parent menu, when this menu is dismissed
        _ = parent_menu.update(cx, |view, cx| {
            view.selected_index = None;
            view.dismiss(&Cancel, window, cx);
        });
    }

    fn handle_dismiss(
        &mut self,
        position: &Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Do not dismiss, if click inside the parent menu
        if let Some(parent) = self.parent_menu.as_ref() {
            if let Some(parent) = parent.upgrade() {
                if parent.read(cx).bounds.contains(position) {
                    return;
                }
            }
        }

        self.dismiss(&Cancel, window, cx);
    }

    fn on_mouse_down_out(
        &mut self,
        e: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.handle_dismiss(&e.position, window, cx);
    }

    fn render_key_binding(
        &self,
        action: Option<Box<dyn Action>>,
        window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Kbd> {
        let action = action?;

        match self
            .action_context
            .as_ref()
            .and_then(|handle| Kbd::binding_for_action_in(action.as_ref(), handle, window))
        {
            Some(kbd) => Some(kbd),
            // Fallback to App level key binding
            None => Kbd::binding_for_action(action.as_ref(), None, window),
        }
        .map(|this| {
            this.p_0()
                .flex_nowrap()
                .border_0()
                .bg(gpui::transparent_white())
        })
    }

    fn render_icon_slot(
        has_icon: bool,
        checked: bool,
        icon: Option<Icon>,
        icon_element: Option<Rc<dyn Fn(&mut Window, &mut App) -> AnyElement + 'static>>,
        icon_png: Option<Arc<Vec<u8>>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        if !has_icon {
            return None;
        }

        if let Some(builder) = icon_element {
            return Some(
                div()
                    .w(ICON_SLOT_SIZE)
                    .h(ICON_SLOT_SIZE)
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(builder(window, cx)),
            );
        }

        if let Some(png) = icon_png {
            return Some(
                div()
                    .w(ICON_SLOT_SIZE)
                    .h(ICON_SLOT_SIZE)
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        img(Arc::new(Image::from_bytes(
                            ImageFormat::Png,
                            (*png).clone(),
                        )))
                        .size(ICON_SLOT_SIZE)
                        .object_fit(ObjectFit::Contain),
                    ),
            );
        }

        let icon = if let Some(icon) = icon {
            icon.clone()
        } else if checked {
            Icon::new(IconName::Check)
        } else {
            Icon::empty()
        };

        Some(
            div()
                .w(ICON_SLOT_SIZE)
                .h(ICON_SLOT_SIZE)
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(icon.xsmall()),
        )
    }

    #[inline]
    fn max_width(&self) -> Pixels {
        self.max_width.unwrap_or(px(500.))
    }

    /// Choose the horizontal side (and offset) for a child submenu flyout.
    ///
    /// We only decide left-vs-right here and always use a *top* anchor. The
    /// vertical fit (flip up near the bottom edge, snap fully into the window
    /// for a tall scrolling menu) is delegated to gpui's `anchored` element,
    /// whose default `SwitchAnchor` fit mode flips the vertical corner when the
    /// flyout would overflow and otherwise snaps it inside the viewport. Because
    /// the flyout is laid out as a child of its trigger row, gpui anchors it to
    /// that row automatically, so it always appears next to «显示更多选项»
    /// instead of being pushed away from it.
    fn update_submenu_menu_anchor(&mut self, window: &Window, cx: &App) {
        let menu_width = self.bounds.size.width;
        if menu_width <= px(0.) {
            return;
        }

        let child_max_width = self
            .selected_index
            .and_then(|ix| match self.menu_items.get(ix) {
                Some(PopupMenuItem::Submenu { menu, .. }) => Some(menu.read(cx).max_width()),
                _ => None,
            })
            .unwrap_or_else(|| self.max_width());

        const FLYOUT_OVERLAP: Pixels = px(8.);
        let window_width = window.viewport_size().width;
        let open_left =
            self.bounds.origin.x + menu_width + child_max_width > window_width;

        self.submenu_anchor = if open_left {
            (Anchor::TopRight, -px(16.))
        } else {
            (Anchor::TopLeft, menu_width - FLYOUT_OVERLAP)
        };
    }

    fn render_item(
        &self,
        ix: usize,
        item: &PopupMenuItem,
        options: RenderOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> MenuItemElement {
        let has_left_icon = options.has_left_icon;
        let is_left_check = options.check_side.is_left() && item.is_checked();
        let right_check_icon = if options.check_side.is_right() && item.is_checked() {
            Some(Icon::new(IconName::Check).xsmall())
        } else {
            None
        };

        let selected = self.selected_index == Some(ix);
        const INNER_PADDING: Pixels = px(8.);

        let is_submenu = matches!(item, PopupMenuItem::Submenu { .. });
        let group_name = format!("{}:item-{}", cx.entity().entity_id(), ix);

        let item_height = match self.size {
            Size::Small => px(24.),
            _ => self.item_row_height(),
        };
        let radius = match self.size {
            Size::Small => options.radius.half(),
            _ => options.radius,
        };

        let this = MenuItemElement::new(ix, &group_name)
            .relative()
            .text_sm()
            .py_0()
            .px(INNER_PADDING)
            .rounded(radius)
            .items_center()
            // Pin the row to its natural height. The items container is a flex
            // column with a `max_h`; with the default `flex-shrink: 1` the rows
            // would be squashed to fit that cap (losing the 32px row height)
            // instead of overflowing and scrolling. `flex_shrink_0` forces the
            // column to overflow so `overflow_y_scroll` can take over.
            .flex_shrink_0()
            .selected(selected)
            .on_hover(cx.listener(move |this, hovered, _, cx| {
                if *hovered {
                    this.selected_index = Some(ix);
                } else if !is_submenu && this.selected_index == Some(ix) {
                    // TODO: Better handle the submenu unselection when hover out
                    this.selected_index = None;
                }

                cx.notify();
            }));

        match item {
            PopupMenuItem::Separator => this
                .h_auto()
                .p_0()
                .my_0p5()
                .mx_neg_1()
                .border_b(px(2.))
                .border_color(cx.theme().border)
                .disabled(true),
            PopupMenuItem::Label(label) => this.disabled(true).cursor_default().child(
                h_flex()
                    .cursor_default()
                    .items_center()
                    .gap_x_1()
                    .children(Self::render_icon_slot(
                        has_left_icon,
                        false,
                        None,
                        None,
                        None,
                        window,
                        cx,
                    ))
                    .child(div().flex_1().child(label.clone())),
            ),
            PopupMenuItem::ElementItem {
                render,
                icon,
                icon_element,
                disabled,
                ..
            } => this
                .when(!disabled, |this| {
                    this.on_click(
                        cx.listener(move |this, _, window, cx| this.on_click(ix, window, cx)),
                    )
                })
                .disabled(*disabled)
                .child(
                    h_flex()
                        .flex_1()
                        .h(item_height)
                        .items_center()
                        .gap_x_1()
                        .children(Self::render_icon_slot(
                            has_left_icon,
                            is_left_check,
                            icon.clone(),
                            icon_element.clone(),
                            None,
                            window,
                            cx,
                        ))
                        .child((render)(window, cx))
                        .children(right_check_icon.map(|icon| icon.ml_3())),
                ),
            PopupMenuItem::Item {
                icon,
                icon_element,
                icon_png,
                label,
                action,
                disabled,
                is_link,
                ..
            } => {
                let show_link_icon = *is_link && self.external_link_icon;
                let action = action.as_ref().map(|action| action.boxed_clone());
                let key = self.render_key_binding(action, window, cx);

                this.when(!disabled, |this| {
                    this.on_click(
                        cx.listener(move |this, _, window, cx| this.on_click(ix, window, cx)),
                    )
                })
                .disabled(*disabled)
                .h(item_height)
                .gap_x_1()
                .children(Self::render_icon_slot(
                    has_left_icon,
                    is_left_check,
                    icon.clone(),
                    icon_element.clone(),
                    icon_png.clone(),
                    window,
                    cx,
                ))
                .child(
                    h_flex()
                        .w_full()
                        .gap_3()
                        .items_center()
                        .justify_between()
                        .when(!show_link_icon, |this| this.child(label.clone()))
                        .children(right_check_icon)
                        .when(show_link_icon, |this| {
                            this.child(
                                h_flex()
                                    .w_full()
                                    .justify_between()
                                    .gap_1p5()
                                    .child(label.clone())
                                    .child(
                                        Icon::new(IconName::ExternalLink)
                                            .xsmall()
                                            .text_color(cx.theme().muted_foreground),
                                    ),
                            )
                        })
                        .children(key),
                )
            }
            PopupMenuItem::Submenu {
                icon,
                icon_element,
                icon_png,
                label,
                menu,
                disabled,
            } => this
                .selected(selected)
                .disabled(*disabled)
                .h(item_height)
                .gap_x_1()
                .child(
                    h_flex()
                        .w_full()
                        .h_full()
                        .items_center()
                        .gap_x_1()
                        .children(Self::render_icon_slot(
                            has_left_icon,
                            false,
                            icon.clone(),
                            icon_element.clone(),
                            icon_png.clone(),
                            window,
                            cx,
                        ))
                        .child(
                            h_flex()
                                .flex_1()
                                .gap_2()
                                .items_center()
                                .justify_between()
                                .child(label.clone())
                                .child(
                                    Icon::new(IconName::ChevronRight)
                                        .xsmall()
                                        .text_color(cx.theme().muted_foreground),
                                ),
                        ),
                )
                .when(selected, |this| {
                    this.child({
                        let (anchor, left) = self.submenu_anchor;
                        let is_bottom_pos =
                            matches!(anchor, Anchor::BottomLeft | Anchor::BottomRight);
                        // Wrap the submenu flyout in `deferred` so it is painted
                        // after (and on top of) the rest of the tree, with the
                        // content mask reset. Without this, a `scrollable` parent
                        // menu (`overflow_y_scroll`) would clip the flyout to its
                        // own scroll rect and the submenu would be invisible —
                        // this is the fix that lets sub-menus work while the
                        // parent scrolls. `with_priority(1)` keeps nested
                        // submenus stacked above their ancestors.
                        deferred(
                            anchored()
                                .anchor(anchor)
                                .child(
                                    div()
                                        .id("submenu")
                                        .occlude()
                                        .when(is_bottom_pos, |this| this.bottom_0())
                                        .when(!is_bottom_pos, |this| this.top_neg_1())
                                        .left(left)
                                        .child(menu.clone()),
                                ),
                        )
                        .with_priority(1)
                    })
                }),
        }
    }
}

impl FluentBuilder for PopupMenu {}
impl EventEmitter<DismissEvent> for PopupMenu {}
impl Focusable for PopupMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[derive(Clone, Copy)]
struct RenderOptions {
    has_left_icon: bool,
    check_side: Side,
    radius: Pixels,
}

impl Render for PopupMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.update_submenu_menu_anchor(window, cx);

        let view = cx.entity().clone();
        let items_count = self.menu_items.len();

        // Clamp the requested cap to the *drawable viewport* (not the OS window
        // bounds, which include the title bar). gpui's `anchored` snaps the
        // flyout into `window.viewport_size()`, so a cap taller than the viewport
        // could never actually be displayed — the menu would overflow and be
        // clipped instead of sitting at the intended 0.92×-window height.
        let viewport_height = window.viewport_size().height;
        let max_height = self
            .max_height
            .unwrap_or_else(|| {
                let window_half_height = window.window_bounds().get_bounds().size.height * 0.5;
                window_half_height.min(px(450.))
            })
            .min(viewport_height - px(8.));

        // Decide here, up front, whether to turn on the scroll viewport and
        // scrollbar. We only do so when the menu opted into `scrollable` *and*
        // its estimated content is taller than `max_height`. This is the key to
        // avoiding a spurious scrollbar on small (e.g. deeply nested) submenus:
        // when the content fits we render a plain menu — no `overflow_y_scroll`
        // viewport (so nothing can be clipped) and no scrollbar layer.
        let needs_scroll = self.scrollable && self.estimated_content_height() > max_height;

        let has_left_icon = self
            .menu_items
            .iter()
            .any(|item| item.has_left_icon(self.check_side));

        let max_width = self.max_width();
        let options = RenderOptions {
            has_left_icon,
            check_side: self.check_side,
            radius: MENU_ITEM_RADIUS,
        };

        v_flex()
            .id("popup-menu")
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::dismiss))
            .on_mouse_down_out(cx.listener(Self::on_mouse_down_out))
            .popover_style(cx)
            .rounded(MENU_CONTAINER_RADIUS)
            .text_color(cx.theme().popover_foreground)
            .relative()
            .occlude()
            .child(
                v_flex()
                    .id("items")
                    .p(MENU_EDGE_PADDING)
                    .gap(px(2.))
                    .min_w(self.min_width.unwrap_or(DEFAULT_POPUP_MENU_MIN_WIDTH))
                    .max_w(max_width)
                    // When the content overflows, pin the items area to exactly
                    // `max_height` (0.92 × window for the shell menu) and scroll;
                    // otherwise size to content and stay fully visible.
                    .when(needs_scroll, |this| {
                        this.h(max_height)
                            .max_h(max_height)
                            .pr(MENU_SCROLLBAR_WIDTH + MENU_SCROLLBAR_GAP)
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                    })
                    .children(
                        self.menu_items
                            .iter()
                            .enumerate()
                            // Ignore last separator
                            .filter(|(ix, item)| !(*ix + 1 == items_count && item.is_separator()))
                            .map(|(ix, item)| self.render_item(ix, item, options, window, cx)),
                    )
                    .on_prepaint(move |bounds, _, cx| view.update(cx, |r, _| r.bounds = bounds)),
            )
            // Scrollbar overlay, rendered as a sibling of the items area that
            // covers the whole popover. Only present when the menu overflows.
            // Sub-menus still display while this is active because each flyout
            // is `deferred` (see `render_item`) and therefore painted outside
            // this clip rect.
            .when(needs_scroll, |this| {
                let mut scrollbar = MenuScrollbar::vertical(&self.scroll_handle);
                // `ScrollbarShow::Always` suppresses the idle fade-out so the
                // bar stays put while the user reads a long menu.
                if self.scrollbar_always {
                    scrollbar = scrollbar.scrollbar_show(ScrollbarShow::Always);
                }
                this.child(
                    div()
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom_0()
                        .w(MENU_SCROLLBAR_WIDTH)
                        .child(scrollbar),
                )
            })
    }
}
