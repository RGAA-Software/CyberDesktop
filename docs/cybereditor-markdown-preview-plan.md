# CyberEditor Markdown Preview 实施计划

## 1. 技术方案

### 1.1 架构

采用**自建轻量渲染引擎**（方案A），核心架构：

```
[editor-text-engine Document] ──text_changed──→ [MarkdownParser]
                                                      ↓
[MarkdownPreviewView] ←────render──── [MarkdownElementBuilder]
         ↓                                      ↑
    gpui-component                    pulldown-cmark
    (div/text/img/scroll)
```

### 1.2 依赖

新增依赖（均为 crates.io 独立包）：
- `pulldown-cmark = "0.12"` — markdown 解析
- `pulldown-cmark-to-cmark = "0.13"` — 序列化回 markdown（checkbox 编辑）
- `comrak = "0.31"` 或只用 `pulldown-cmark` — 备选

复用现有依赖：
- `syntect` — 代码块语法高亮（已有）
- `gpui-component` — UI 组件
- `editor-text-engine` — 文档模型

### 1.3 不支持（V1 范围）

- HTML 块渲染（常见 markdown 不需要）
- Mermaid 图表（后续可加）
- 数学公式（后续可加）
- Footnote（后续可加）

---

## 2. 实施阶段

### Phase 1: 基础设施（~2h）

1. **新增 crate `markdown-preview-engine`**
   - 位置：`crates/markdown-preview-engine/`
   - 职责：markdown 解析 + GPUI 元素构建（纯渲染，不耦合编辑器）
   - 导出：`MarkdownDocument`, `MarkdownElement`, `MarkdownStyle`

2. **新增 crate `markdown-preview-ui`**
   - 位置：`crates/markdown-preview-ui/`
   - 职责：preview 视图组件 + 编辑器绑定逻辑
   - 依赖：`markdown-preview-engine`, `editor-text-engine`, `editor-ui`

3. **注册到 workspace**
   - `Cargo.toml` workspace members
   - `editor-app/Cargo.toml` 依赖 `markdown-preview-ui`
   - `editor-ui/Cargo.toml` 依赖 `markdown-preview-ui`

### Phase 2: 解析与渲染核心（~4h）

4. **实现 `MarkdownParser`**
   - 用 `pulldown-cmark` 解析 markdown 文本
   - 产出结构化数据：`Vec<MarkdownBlock>`
   - 保留 source byte range（每个 block 记录对应原文位置）

5. **定义 `MarkdownBlock` 类型**
   ```rust
   enum MarkdownBlock {
       Heading(u8, Vec<MarkdownInline>, Range<usize>),
       Paragraph(Vec<MarkdownInline>, Range<usize>),
       CodeBlock(Option<String>, String, Range<usize>), // lang, code, range
       List(Vec<MarkdownListItem>, Range<usize>),
       BlockQuote(Vec<MarkdownBlock>, Range<usize>),
       Rule(Range<usize>),
   }
   ```

6. **实现 `MarkdownElementBuilder`**
   - 将 `Vec<MarkdownBlock>` 转换为 GPUI 元素树
   - 使用 `gpui-component` 的 `div`, `v_flex`, `h_flex`, `StyledText`
   - 代码块用 `syntect` 做语法高亮，生成 ` StyledText`

7. **实现 `MarkdownStyle`**
   - 从当前 theme 读取颜色（primary, foreground, muted, accent 等）
   - 支持 light/dark 模式切换

### Phase 3: 编辑器绑定（~3h）

8. **实现 `MarkdownPreviewView`**
   - GPUI `Render` 实体
   - 包含 `ScrollHandle` 用于滚动
   - 显示 `MarkdownElement`

9. **编辑器事件订阅**
   - 监听 `editor-text-engine` 的文档变化
   - 200ms debounce 后重新解析并渲染
   - 监听编辑器 selection 变化，preview 滚动到对应位置

10. **双向跳转**
    - preview 中点击段落/heading → 编辑器跳转对应行
    - 编辑器中光标移动 → preview 自动滚动高亮对应块

### Phase 4: UI 集成（~2h）

11. **命令与快捷键**
    - `ToggleMarkdownPreview` action
    - 快捷键：`Ctrl+Shift+V`（打开/关闭 preview）
    - 菜单项：View → Markdown Preview

12. **窗口/面板布局**
    - 右侧分割面板显示 preview
    - 与编辑器同窗口，可拖动调整宽度
    - 关闭 editor tab 时同步关闭 preview

13. **编辑器状态栏按钮**
    - 状态栏添加 "Preview" 按钮（类似 zed 的 eye 图标）
    - 点击打开/关闭 preview

### Phase 5: 测试与打磨（~2h）

14. **基本功能测试**
    - 打开 `.md` 文件自动识别
    - 编辑时 preview 实时更新
    - 支持 heading、paragraph、list、code block、blockquote、link、image、rule

15. **边界测试**
    - 大文件性能（>10MB）
    - 中文内容
    - 特殊字符
    - 代码块语法高亮

16. **视觉打磨**
    - 调整 padding、margin、字体大小
    - 确保 light/dark 主题下都正常
    - 滚动条样式统一

---

## 3. 验收标准

### 3.1 功能验收

| 功能 | 标准 |
|------|------|
| 打开 markdown | `.md` / `.markdown` 文件打开时，可以唤起 preview |
| 快捷键 | `Ctrl+Shift+V` 切换 preview 显示/隐藏 |
| 实时预览 | 编辑 markdown 内容，preview 在 200ms 内更新 |
| 双向跳转 | preview 中点击 → 编辑器光标跳到对应位置；编辑器光标移动 → preview 滚动高亮 |
| 代码高亮 | 代码块显示语法高亮（syntect） |
| 图片显示 | 本地相对路径图片正常显示 |
| 主题适配 | light/dark 模式下 preview 颜色正确切换 |

### 3.2 性能验收

- 100KB markdown 文件：解析 + 渲染 < 50ms
- 1MB markdown 文件：解析 + 渲染 < 200ms
- 编辑时 debounce 200ms，不卡顿

### 3.3 代码验收

- 无 compiler warnings
- `cargo clippy` 通过
- 新增 crate 有基本单元测试（解析器测试）

---

## 4. 风险与应对

| 风险 | 应对 |
|------|------|
| `pulldown-cmark` API 与 GPUI 元素映射复杂 | 先支持常用 block 类型，不常用的 fallback 为纯文本 |
| source-index 双向映射实现困难 | Phase 3 中先做单向（editor→preview），双向作为增强 |
| `gpui-component` Scrollable 与自定义元素冲突 | 用 `div().overflow_y_scroll()` 代替，手动处理 scroll handle |
| 图片加载异步复杂 | V1 只支持本地相对路径图片，用 `std::fs::read` 同步读取 |

---

## 5. 实施计划时间表

| 阶段 | 预计时间 | 交付物 |
|------|---------|--------|
| Phase 1 | 2h | 新增 crate 注册完毕，可编译通过 |
| Phase 2 | 4h | markdown 解析 + 渲染核心完成，可显示静态 markdown |
| Phase 3 | 3h | 编辑器绑定 + 双向跳转 |
| Phase 4 | 2h | UI 集成（快捷键、按钮、面板） |
| Phase 5 | 2h | 测试打磨，零 warning |
| **总计** | **~13h** | 完整功能 |

---

*计划制定时间：2026-06-07*
*实施人：Kimi*
