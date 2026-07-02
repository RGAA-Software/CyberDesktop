use super::*;

impl FileBrowser {
    /// Prefetch Shell context menu on the dedicated Shell STA worker (non-blocking for GPUI).
    pub(super) fn request_shell_menu_fetch(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !matches!(
            self.browse_location,
            BrowseLocation::Directory | BrowseLocation::FileTag { .. }
        ) {
            return;
        }

        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }

        let extended = self.context_menu_extended_verbs;
        let paths_key = normalize_paths_for_shell_cache(&paths);
        if self
            .shell_menu_cache
            .read()
            .ok()
            .and_then(|guard| {
                guard.as_ref().map(|cache| {
                    cache.paths == paths_key
                        && cache.extended_verbs == extended
                        && !cache.entries.is_empty()
                })
            })
            .unwrap_or(false)
        {
            return;
        }

        if self.shell_menu_fetch_paths.as_ref() == Some(&paths_key) {
            return;
        }

        self._shell_menu_task.take();
        self.shell_menu_fetch_paths = Some(paths_key.clone());
        self.shell_menu_fetch_generation = self.shell_menu_fetch_generation.wrapping_add(1);
        let fetch_generation = self.shell_menu_fetch_generation;
        let browser_handle = cx.weak_entity();

        let menu_icon_extract_px = platform::menu_icon_pixel_size(window.scale_factor());
        let paths_for_query = paths_key.clone();
        let paths_for_retry = paths_key.clone();
        self._shell_menu_task = Some(cx.spawn(async move |this, cx| {
            let query_result = cx
                .background_spawn(async move {
                    platform::query_shell_context_menu_items(
                        &paths_for_query,
                        extended,
                        menu_icon_extract_px,
                    )
                })
                .await;
            let retry_after_err = query_result.is_err();

            let menu_open = this
                .update(cx, |browser, cx| {
                    browser.shell_menu_fetch_paths = None;
                    if fetch_generation != browser.shell_menu_fetch_generation {
                        return false;
                    }
                    match query_result {
                        Ok(entries) => {
                            if let Ok(mut guard) = browser.shell_menu_cache.write() {
                                *guard = Some(ShellMenuCache {
                                    paths: paths_key,
                                    extended_verbs: extended,
                                    entries,
                                });
                            }
                        }
                        Err(error) => {
                            tracing::warn!(
                                target: "shell_menu",
                                paths = ?paths_key,
                                extended,
                                error = ?error,
                                "fetch failed; cache cleared for retry"
                            );
                            if let Ok(mut guard) = browser.shell_menu_cache.write() {
                                *guard = None;
                            }
                        }
                    }
                    browser.shell_menu_revision = browser.shell_menu_revision.wrapping_add(1);
                    let open = browser.context_menu_open;
                    cx.notify();
                    open
                })
                .unwrap_or(false);

            if menu_open {
                let handle = browser_handle.clone();
                let _ = this.update(cx, |_, cx| {
                    cx.defer(move |cx| {
                        let Some(window) = cx.active_window() else {
                            return;
                        };
                        let _ = window.update(cx, |_, window, cx| {
                            FileBrowser::install_context_menu_flyout(&handle, window, cx, false);
                        });
                    });
                });
                if retry_after_err {
                    let paths_retry = paths_for_retry.clone();
                    let retry_handle = browser_handle.clone();
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(2))
                        .await;
                    let _ = this.update(cx, |browser, cx| {
                        if !browser.context_menu_open {
                            return;
                        }
                        let selection =
                            normalize_paths_for_shell_cache(&browser.selected_paths_vec());
                        if selection != paths_retry {
                            return;
                        }
                        let cache_hit = browser
                            .shell_menu_cache
                            .read()
                            .ok()
                            .map(|g| g.is_some())
                            .unwrap_or(false);
                        if cache_hit {
                            return;
                        }
                        let handle = retry_handle.clone();
                        cx.defer(move |cx| {
                            let Some(window) = cx.active_window() else {
                                return;
                            };
                            let _ = window.update(cx, |_, window, cx| {
                                let _ = handle.update(cx, |browser, cx| {
                                    browser.request_shell_menu_fetch(window, cx);
                                    cx.notify();
                                });
                            });
                        });
                    });
                    let _ = this.update(cx, |_, cx| {
                        cx.defer(move |cx| {
                            let Some(window) = cx.active_window() else {
                                return;
                            };
                            let _ = window.update(cx, |_, window, cx| {
                                FileBrowser::install_context_menu_flyout(
                                    &retry_handle,
                                    window,
                                    cx,
                                    false,
                                );
                            });
                        });
                    });
                }
            }
        }));
    }

    pub(super) fn dismiss_context_menu(&mut self) {
        self.context_menu_open = false;
        self.context_menu_view = None;
        self._context_menu_subscription = None;
    }

    pub(super) fn open_context_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.context_menu_position = position;
        self.context_menu_open = true;
        #[cfg(windows)]
        {
            if platform::peek_shell_new_menu_items().is_none() {
                let browser_handle = cx.weak_entity();
                let _ = cx.spawn(async move |this, cx| {
                    cx.background_spawn(async move {
                        platform::refresh_shell_new_menu_cache();
                    })
                    .await;
                    let menu_open = this
                        .update(cx, |browser, cx| {
                            if browser.context_menu_open {
                                browser.shell_menu_revision =
                                    browser.shell_menu_revision.wrapping_add(1);
                            }
                            cx.notify();
                            browser.context_menu_open
                        })
                        .unwrap_or(false);
                    if menu_open {
                        let handle = browser_handle.clone();
                        let _ = this.update(cx, |_, cx| {
                            cx.defer(move |cx| {
                                let Some(window) = cx.active_window() else {
                                    return;
                                };
                                let _ = window.update(cx, |_, window, cx| {
                                    FileBrowser::install_context_menu_flyout(
                                        &handle,
                                        window,
                                        cx,
                                        false,
                                    );
                                });
                            });
                        });
                    }
                });
            } else {
                platform::warm_shell_new_menu_cache();
            }
        }
        self.request_shell_menu_fetch(window, cx);
        self.schedule_context_menu_rebuild(window, cx);
        cx.notify();
    }

    /// Rebuild flyout after the current `FileBrowser` update finishes (avoids `double_lease_panic`).
    pub(super) fn schedule_context_menu_rebuild(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.context_menu_open {
            return;
        }
        let browser_handle = cx.weak_entity();
        window.defer(cx, move |window, cx| {
            Self::install_context_menu_flyout(&browser_handle, window, cx, false);
        });
    }

    /// Build `PopupMenu` outside any `FileBrowser::update`, then attach it in a short update.
    pub(super) fn install_context_menu_flyout(
        browser_handle: &WeakEntity<Self>,
        window: &mut Window,
        cx: &mut App,
        only_if_revision_changed: bool,
    ) {
        let Some(browser_entity) = browser_handle.upgrade() else {
            return;
        };
        let (open, needs_rebuild) = {
            let browser = browser_entity.read(cx);
            (
                browser.context_menu_open,
                browser.context_menu_built_revision != browser.shell_menu_revision,
            )
        };
        if !open || (only_if_revision_changed && !needs_rebuild) {
            return;
        }

        let menu = PopupMenu::build(window, cx, {
            let browser_entity = browser_entity.clone();
            move |menu, window, cx| {
                context_menu::build_context_menu(menu, browser_entity, window, cx)
            }
        });

        let browser_weak = browser_entity.downgrade();
        let _ = browser_weak.update(cx, |browser, cx| {
            if !browser.context_menu_open {
                return;
            }
            let dismiss_weak = browser_weak.clone();
            browser._context_menu_subscription = Some(window.subscribe(&menu, cx, {
                move |_, _: &DismissEvent, window, cx| {
                    let _ = dismiss_weak.update(cx, |browser, cx| {
                        browser.dismiss_context_menu();
                        cx.notify();
                    });
                    window.refresh();
                }
            }));
            browser.context_menu_view = Some(menu.clone());
            browser.context_menu_built_revision = browser.shell_menu_revision;
            menu.focus_handle(cx).focus(window, cx);
            cx.notify();
        });
    }

    pub(super) fn render_context_menu_overlay(&self, window: &Window) -> impl IntoElement {
        let Some(menu) = self.context_menu_view.clone() else {
            return div().into_any_element();
        };
        let position = self.context_menu_position;

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

    pub(super) fn prepare_context_menu_target(&mut self, index: usize) {
        let Some(item) = self.display_items.get(index) else {
            return;
        };
        let path = item.path.clone();
        if !self.selected_paths.contains(&path) {
            self.selected_paths.clear();
            self.selected_paths.insert(path);
            self.anchor_index = Some(index);
            self.focused_index = Some(index);
        }
    }

    pub(super) fn prepare_column_context_menu_target(&mut self, col_index: usize, index: usize) {
        let Some(item) = self
            .column_listings
            .get(col_index)
            .and_then(|items| items.get(index))
        else {
            return;
        };

        let path = item.path.clone();
        if !self.selected_paths.contains(&path) {
            self.selected_paths.clear();
            self.selected_paths.insert(path.clone());
            self.column_selected_path = Some((col_index, path));
            self.anchor_index = Some(index);
            self.focused_index = Some(index);
        }
        self.active_column_index = Some(col_index);
    }
}
