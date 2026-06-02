# CyberEditor `editor_view` 模块拆分计划

`crates/editor-ui/src/editor_view/` 由原 ~4200 行单文件拆成可维护子模块。对外 API 不变：`editor_ui::EngineEditor`。

## 目标目录（已落地）

```
crates/editor-ui/src/editor_view/
├── mod.rs                 # 模块根 + re-export
├── imports.rs             # 子模块共享 use
├── editor.rs              # EngineEditor + CloseTarget + new/view
├── language.rs
├── text_util.rs
├── state/
├── impl/                  # 按域拆分的 impl EngineEditor（方法 pub(crate)）
├── ui/                    # 面板 / 滚动条 / chrome
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

### 阶段 0：模块骨架

- [x] 创建 `editor_view/` 目录
- [x] `lib.rs`：`pub mod editor_view`
- [x] 构建通过

### 阶段 1：Canvas + 绘制工具

- [x] `canvas/syntax_paint.rs`、`text_util.rs`
- [x] `canvas/` 拆为 `element`、`prepaint`、`prepaint_wrapped`、`paint`、`syntax_paint`
- [x] `comment_prefix` 在 `language.rs`

### 阶段 2：状态类型

- [x] `state/` 各类型
- [x] `language.rs`
- [x] `editor.rs` 结构体与构造函数

### 阶段 3：`impl EngineEditor` 按域拆分

- [x] `impl/core.rs` … `impl/mouse_scroll.rs`（共 16 个文件）

### 阶段 4：UI 渲染层

- [x] `ui/widgets.rs`、`chrome.rs`、`overlays.rs`、`find_bar.rs`、`search_panel_ui.rs`、`goto_bar.rs`、`scrollbars.rs`
- [x] `render.rs`、`input_handler.rs`

### 阶段 5：收尾

- [x] `mod.rs` 仅模块声明与 `pub use`
- [x] `cargo build -p editor-ui` 与 `--bin cyber_editor` 通过
- [ ] 手动冒烟（编辑、软换行、查找、标签页、关闭确认）

## 原则

- 同 crate 内多个 `impl EngineEditor`；跨文件可调用的方法使用 `pub(crate)`。
- `state/` 无 GPUI 绘制；`ui/` 只拼 Div；`impl/` 放行为。
- `canvas/` 子模块拆分已完成。

## 建议 git 提交（已完成第 1 次）

1. `refactor(ui-editor): split editor_view into canvas, state, language modules` — 903f32c
2. `refactor(ui-editor): split EngineEditor impl, UI, render, input_handler` — 待提交
