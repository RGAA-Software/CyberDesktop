# CyberEditor `editor_view` 模块拆分计划

`crates/ui-editor/src/editor_view.rs` 约 4200+ 行，需按功能拆成可维护子模块。对外 API 不变：`cyberfiles_ui_editor::EngineEditor`。

## 目标目录

```
crates/ui-editor/src/
├── lib.rs
└── editor_view/
    ├── mod.rs
    ├── language.rs
    ├── editor.rs                    # EngineEditor struct + new/view
    ├── state/
    │   ├── mod.rs
    │   ├── input_target.rs
    │   ├── find.rs
    │   ├── search_panel.rs
    │   ├── tab.rs
    │   └── scroll.rs
    ├── impl/
    │   ├── core.rs
    │   ├── movement.rs
    │   ├── selection.rs
    │   ├── file_io.rs
    │   ├── tabs.rs
    │   ├── close_confirm.rs
    │   ├── recent.rs
    │   ├── disk_watch.rs
    │   ├── clipboard.rs
    │   ├── editing.rs
    │   ├── goto.rs
    │   ├── search_panel.rs
    │   ├── find.rs
    │   ├── keyboard.rs
    │   └── mouse_scroll.rs
    ├── ui/
    │   ├── widgets.rs
    │   ├── chrome.rs
    │   ├── overlays.rs
    │   ├── find_bar.rs
    │   ├── search_panel_ui.rs
    │   ├── goto_bar.rs
    │   └── scrollbars.rs
    ├── render.rs
    ├── input_handler.rs
    └── canvas/
        ├── mod.rs
        ├── element.rs
        ├── prepaint.rs
        ├── prepaint_wrapped.rs
        ├── paint.rs
        └── syntax_paint.rs
```

## 任务清单

### 阶段 0：模块骨架（无行为变更）

- [x] 创建 `editor_view/` 目录，将 `editor_view.rs` 移为 `editor_view/mod.rs`
- [x] 更新 `lib.rs`：`pub mod editor_view`
- [x] `cargo build -p cyberfiles-ui-editor` 通过

### 阶段 1：Canvas + 绘制工具（~900 行）

- [x] 抽出 `canvas/syntax_paint.rs`：`build_runs`、`kind_color`、`word_occurrences`、`occurrence_word`、`shape_one_wrapped`、`measure_rows`（`char_to_byte` 在 `text_util.rs`）
- [x] `canvas/mod.rs`：`EditorCanvas`、`CanvasPrepaint`、`VisibleRow`、`WrappedRow`、`impl Element`（单文件，未再拆 element/prepaint/paint）
- [x] `text_util.rs`：`char_to_byte`、`wrap_rows`
- [x] 从 `mod.rs` 移除内联 canvas 与底部绘制辅助函数；`comment_prefix` 暂留 `mod.rs`（`toggle_comment` 使用）
- [ ] 可选后续：拆 `canvas/element.rs`、`prepaint.rs`、`prepaint_wrapped.rs`、`paint.rs`
- [ ] 验收：编辑、软换行、语法高亮、同词高亮、命中测试（需手动冒烟）

### 阶段 2：状态类型（~260 行）

- [x] `state/`：`InputTarget`、`FindState`、`SearchPanelState`、`TabSlot`、滚动/可见行结构体
- [x] `language.rs`：`language_for_path`
- [ ] `editor.rs`：`EngineEditor` 字段 + `new` + `view`（仍留在 `mod.rs`）
- [x] `pub(crate)` 可见性统一（state / canvas 子模块）

### 阶段 3：按域拆分 `impl EngineEditor`（~1800 行）

- [ ] `impl/core.rs` — syntax、changed、caret 可见
- [ ] `impl/movement.rs` — 光标移动
- [ ] `impl/selection.rs` — 多光标、选词/行
- [ ] `impl/file_io.rs` — 异步 open/save/load
- [ ] `impl/tabs.rs` — 标签页 park/activate
- [ ] `impl/close_confirm.rs` — 未保存关闭确认
- [ ] `impl/recent.rs` — MRU
- [ ] `impl/disk_watch.rs` — 外部文件变更
- [ ] `impl/clipboard.rs`
- [ ] `impl/editing.rs` — indent、comment、zoom、wrap
- [ ] `impl/goto.rs`
- [ ] `impl/search_panel.rs` — Find in Files 逻辑
- [ ] `impl/find.rs` — Find/Replace 逻辑
- [ ] `impl/keyboard.rs` — `on_key_down`
- [ ] `impl/mouse_scroll.rs` — 鼠标、滚动条

### 阶段 4：UI 渲染层（~700 行）

- [ ] `ui/widgets.rs` — `bar_button`、`render_input_field`
- [ ] `ui/chrome.rs` — title_bar、header、tab_bar、disk_banner
- [ ] `ui/overlays.rs` — about、shortcuts、recent、close_confirm
- [ ] `ui/find_bar.rs`、`search_panel_ui.rs`、`goto_bar.rs`、`scrollbars.rs`
- [ ] `render.rs` — `impl Render`
- [ ] `input_handler.rs` — `impl EntityInputHandler`

### 阶段 5：收尾

- [ ] `mod.rs` 仅保留模块声明与 `pub use`
- [ ] 删除冗余、补模块级 `//!` 文档
- [ ] 全量 `cargo build -p cyberfiles --bin cybereditor` + 手动冒烟

## 原则

- 同一 crate 内多个 `impl EngineEditor` 块，无需 trait。
- `state/` 无 GPUI 绘制逻辑；`ui/` 只拼 Div；`impl/` 放行为。
- 单文件建议 ≤400 行（canvas 绘制可略超）。
- 每阶段单独提交，便于 review 与 bisect。

## 建议 git 提交信息

1. `refactor(ui-editor): extract EditorCanvas into canvas/`
2. `refactor(ui-editor): extract editor state types`
3. `refactor(ui-editor): split EngineEditor impl by domain`
4. `refactor(ui-editor): extract UI render modules`
5. `refactor(ui-editor): finalize editor_view module tree`
