# CyberEditor 横向视口虚拟化 —— TODO

背景：纵向已对 `display_lines` 做视口虚拟化；单行超长（如 minified JS bundle）仍会对**整行** `line_text` + `shape_line`，导致 UI 卡死。本文档跟踪横向切片方案的实施进度。

原则（与纵向对称）：

```text
scroll_x .. scroll_x + view_w
    → 列区间 [col_start, col_end)
    → rope 片段 + highlights(字节区间) + shape_line(片段)
    → hit-test 带 start_col 偏移
```

---

## Phase 1 — 止血（非 wrap 模式）

- [x] **文档** — 本文件 + 在 `cybereditor-engine-todo.md` 增加索引
- [x] **text-engine** — `line_chars_slice(line, col_start, col_end)`，避免整行 `line_text`
- [x] **ui-editor** — `horizontal_viewport.rs`：等宽列估算、`viewport_col_range`、长行阈值
- [x] **prepaint_normal** — 长行只 shape 视口片段；`content_width` 用 `char_width × line_len` 估算
- [x] **VisibleRow / VisibleLine** — 增加 `start_col`（片段在行内起始列）
- [x] **命中 / IME** — `mouse_scroll`、`input_handler` 按 `start_col` + 片段局部列换算
- [x] **caret reveal** — 长行用等宽估算，不 shape 整行
- [ ] **验证** — 打开几百 KB 单行 bundle 不卡死；普通多行文件行为不变

## Phase 2 — 正确性

- [ ] 行宽前缀缓存（列 → x），编辑时 invalidate 该行
- [ ] caret reveal / 滚动条读缓存，替代等宽估算
- [ ] `refresh_syntax` / `reparse` 移出 UI 线程（或打开大文件时延迟）
- [ ] 超长行检测（>N KB/行）：状态栏提示或建议开启 Word Wrap

## Phase 3 — Soft wrap 路径

- [ ] `prepaint_wrapped`：视口内按**子行**虚拟化，禁止对 300KB 单行一次 `shape_text`
- [ ] 软换行下竖向滚动条子行精度（可选，见 engine-todo）

---

## 验证

```powershell
cargo test -p cyberfiles-text-engine
cargo build -p cyberfiles --bin cybereditor --release
cargo run -p cyberfiles --bin cybereditor --release -- <path-to-minified.js>
```

- 打开 minified 单行：窗口可响应、可横向滚动
- 普通 `.rs` 多行文件：选中、右键、语法高亮正常
