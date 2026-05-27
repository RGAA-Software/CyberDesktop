use std::path::PathBuf;
use std::rc::Rc;

use cyberfiles_fs::{BreadcrumbMenuSection, DirectoryReadOptions, PathBreadcrumb};
use gpui::{App, Window};

/// Callbacks for [`super::breadcrumb_bar::PathBreadcrumbBar`] (built once per main page).
pub struct OmnibarBreadcrumbCallbacks {
    pub show_root: bool,
    pub root_menu: Rc<dyn Fn() -> Vec<BreadcrumbMenuSection>>,
    pub on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    pub on_navigate_new_tab: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    pub on_home: Rc<dyn Fn(&mut Window, &mut App)>,
    pub on_drop_paths: Rc<dyn Fn(PathBuf, Vec<PathBuf>, &mut Window, &mut App)>,
    pub on_drag_hover: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    pub on_show_full_path: Rc<dyn Fn(&mut Window, &mut App)>,
}

impl OmnibarBreadcrumbCallbacks {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        show_root: bool,
        root_menu: Rc<dyn Fn() -> Vec<BreadcrumbMenuSection>>,
        on_navigate: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
        on_navigate_new_tab: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
        on_home: Rc<dyn Fn(&mut Window, &mut App)>,
        on_drop_paths: Rc<dyn Fn(PathBuf, Vec<PathBuf>, &mut Window, &mut App)>,
        on_drag_hover: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
        on_show_full_path: Rc<dyn Fn(&mut Window, &mut App)>,
    ) -> Self {
        Self {
            show_root,
            root_menu,
            on_navigate,
            on_navigate_new_tab,
            on_home,
            on_drop_paths,
            on_drag_hover,
            on_show_full_path,
        }
    }

    pub fn breadcrumb_bar(
        &self,
        segments: Vec<PathBreadcrumb>,
        available_width: f32,
        read_options: DirectoryReadOptions,
        working_directory: Option<PathBuf>,
    ) -> super::breadcrumb_bar::PathBreadcrumbBar {
        super::breadcrumb_bar::PathBreadcrumbBar::new(
            self.show_root,
            segments,
            available_width,
            read_options,
            working_directory,
            self.root_menu.clone(),
            self.on_navigate.clone(),
            self.on_navigate_new_tab.clone(),
            self.on_home.clone(),
            self.on_drop_paths.clone(),
            self.on_drag_hover.clone(),
            self.on_show_full_path.clone(),
        )
    }
}
