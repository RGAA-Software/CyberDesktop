# CyberFiles UI 改版计划

依据交互稿 [`design/cyber_files.html`](../design/cyber_files.html)（迭代至 **V11**）对 CyberFiles 外壳与内容区做视觉对齐。

本文只描述 **UI 表现** 改造，不新增、删除或改变任何业务能力。

---

## 约束（必须遵守）

| # | 约束 | 说明 |
|---|------|------|
| 1 | **不改字体** | 继续使用现有 `IBM Plex Sans`（UI）/ `Lilex`（等宽），不引入 DM Sans，不调整字号体系除非为对齐间距所必需 |
| 2 | **不改功能** | 所有菜单项、快捷键、action、数据流、窗格逻辑保持不变；仅调整布局位置、尺寸、颜色、圆角、边框、阴影等视觉属性 |
| 3 | **图标统一 Tabler** | 全部 UI 图标来自 [Tabler Icons](https://tabler.io/icons)（MIT，24×24，2px stroke）；替换现有 Material Symbols、Lucide（gpui-component 默认）、以及 `ic_*` 扩展彩色 SVG |

### 「只改 UI」的边界

**允许：**

- 调整控件在界面中的 **位置与分组**（例如把视图切换按钮从内容工具栏挪到操作栏），只要对应 action 与快捷键不变
- 调整主题色、间距、行高、圆角、选中态、侧栏宽度等
- 用 Tabler 图标替换 Shell 列表图标与扩展名图标（视觉变化，不改变打开/选择等行为）
- 保留信息面板、设置、GitHub、通知等 **设计稿未画出** 的现有入口（仅做样式统一）

**不允许：**

- 新增/删除菜单项、工具栏按钮、侧栏入口、视图模式
- 修改搜索、分栏、标签、回收站等业务逻辑
- 更换字体族或引入 Web 字体加载流程

---

## 设计稿要点（V11 最终态）

参考 `cyber_files.html` 后半段 CSS 覆盖（V3–V11），核心视觉语言如下。

### 色彩（亮色 / 暗色）

| Token | 亮色 | 暗色 | 用途 |
|-------|------|------|------|
| `bg-base` | `#F6F7F2` | `#10120E` | 窗口底、状态栏 |
| `bg-primary` | `#FFFFFF` | `#1A1D17` | 主内容区、激活标签 |
| `bg-secondary` | `#F0F2EA` | `#14170F` | 侧栏、面包屑底 |
| `bg-hover` | `#E7EBDD` | `#24291D` | 悬停 |
| `text-primary` | `#171A14` | `#E8EDE0` | 主文字 |
| `text-secondary` | `#646B59` | `#A0A895` | 次要文字 |
| `text-tertiary` | `#9BA391` | `#626C59` | 分组标题、辅助 |
| `border` | `rgba(31,45,18,.075)` | `rgba(255,255,255,.07)` | 分隔线 |
| `accent` | `#5C8F18` | `#78B828` | 主强调色 |
| `accent-light` | `#EAF4D9` | `#1E2D13` | 选中背景 |
| `accent-text` | `#284306` | `#C8E998` | 选中文字 |

### 布局尺寸

| 区域 | 高度/宽度 | 备注 |
|------|-----------|------|
| 标题栏 | 46px | Logo + 应用名 + 标签 + 右侧操作 |
| 导航栏 | 52px | 仅浏览目录时显示；含面包屑、分栏、固定 |
| 操作栏 | 48px | 新建、视图模式组、文件命令、排序 |
| 侧栏 | 214px | 可拖拽缩放范围保持现有 resizable 行为 |
| 列表行 | 32px | 详情列表紧凑行高 |
| 状态栏 | 32px | |

### 关键交互视觉

- **标签页**：圆角顶部标签，激活项白底 + 细边框；首页标签无关闭钮
- **侧栏激活项**：白底卡片 + 左侧 3px 绿色竖条（非纯浅色铺满）
- **列表选中**：`accent-light` 背景 + 左侧 3px 竖条
- **视图模式**：操作栏内分段按钮组（列表 / 网格 / 卡片 / 多列）
- **右键菜单**：圆角 14px、半透明背景、行高约 34px（现有 popup menu 逻辑不变，只改样式）

---

## 现状 vs 设计稿（差异摘要）

| 区域 | 设计稿 | 当前实现 | 改版类型 |
|------|--------|----------|----------|
| 主题色 | 暖绿 | Ant / One 等通用主题 | 新增 CyberFiles 主题集或覆盖 token |
| 标题栏 | Logo + 「CyberFiles」文字 | 仅 logo 占位 + 应用菜单 | UI |
| 标题栏标签 | 圆角顶部 / 胶囊风格 | gpui-component TabBar 默认 | UI |
| 导航栏 | 无边框圆角按钮 + 圆角面包屑 | 48px 工具栏 + Omnibar | UI |
| 操作栏 | 独立一行，含视图分段组 | 命令分散在 `content-toolbar` | UI（位置重组） |
| 文件列表 | 32px 行、左选中条 | 较高行、主题默认选中 | UI |
| 侧栏 | 214px、驱动器圆环 | 240px 默认、无圆环 | UI |
| 图标 | Tabler | Material + Lucide + Shell | 图标替换 |
| 信息面板 | 未出现在稿中 | 右侧可折叠 | **保留**，仅样式统一 |

---

## 图标迁移方案（Tabler）

### 原则

1. **单一来源**：[Tabler Icons](https://tabler.io/icons) outline 风格，`stroke-width: 2`，`currentColor`
2. **存放路径**：`crates/app-assets/assets/icons/tabler/<name>.svg`（与旧 Material 图标分离，便于批量替换与清理）
3. **同步脚本**：新增 `scripts/sync_tabler_icons.py`，从 `@tabler/icons` npm 包或 GitHub raw 拉取 SVG（参考现有 `sync_material_icons.py`）
4. **代码入口**：在 `crates/files-ui/src/icons.rs` 集中定义 `tabler_icon(name) -> Icon`，禁止散落硬编码旧路径
5. **弃用**：改版完成后删除不再引用的 Material / `ic_*` SVG；`sync_material_icons.py` 标记为 legacy

### 不替换为 Tabler 的例外

| 类型 | 处理 |
|------|------|
| 文件行 Windows Shell 缩略图/真实图标 | 改为 Tabler **文件类型** 图标 + 设计稿中的浅色底块（`fi.folder` / `fi.video` 等），不再调用 `shell_icon_for_path` 用于列表行（首页卡片可保留 Shell 缩略图或一并统一，见阶段 4） |
| 用户自定义文件标签颜色点 | 保留 CSS/主题色圆点，不用图标 |

### UI 图标映射表（Material / Lucide → Tabler）

与 `design/cyber_files.html` 中 `ti ti-*` 类名对齐。

| 用途 | 当前资源 | Tabler 图标名 |
|------|----------|---------------|
| 应用 Logo | `content_copy.svg` 占位 | `files` |
| 首页 | `ic_home` / `LayoutDashboard` | `home` |
| 文件夹 | `folder` / Shell | `folder` / `folder-filled` |
| 新建标签 | `plus` | `plus` |
| 关闭标签 | Lucide `close` | `x` |
| 主题暗色 | Lucide `moon` | `moon` |
| 主题亮色 | Lucide `sun` | `sun` |
| 设置 | `settings-2` | `settings` |
| GitHub | Lucide `github` | `brand-github` |
| 通知 | `bell` | `bell` |
| 后退/前进/上级 | `arrow-*` | `arrow-left` / `arrow-right` / `arrow-up` |
| 刷新 | `redo-2` | `refresh` |
| 分栏 | `splitscreen` | 自定义两格或 `layout-columns` |
| 固定目录 | `pin` | `pin` / `pinned` |
| 信息面板开关 | `panel-right-*` | `layout-sidebar-right` / `layout-sidebar-right-collapse` |
| 复制/剪切/粘贴 | `content_*` | `copy` / `cut` / `clipboard` |
| 重命名 | `drive_file_rename_outline` | `pencil` |
| 删除 | `delete` | `trash` |
| 新建文件夹/文件 | `create_new_folder` / `note_add` | `folder-plus` / `file-plus` |
| 详情视图 | `view_headline` | `list-details` |
| 列表视图 | `PanelLeftOpen` | `list` |
| 网格视图 | `layout-dashboard` | `layout-grid` |
| 卡片视图 | `view_cozy` | `layout-board` |
| 多列视图 | `PanelLeft` | `columns-3` |
| 回收站 | `inbox` / `delete` | `trash` |
| 还原 | `restore_deleted` | `arrow-back-up` |
| 属性/信息 | `info` | `info-circle` |
| 标签 | `label` | `tag` |
| 压缩/解压 | `folder_zip` | `file-zip` |
| 在 Explorer 打开 | `FolderOpen` | `folder-open` |
| 外部打开 | `external-link` | `external-link` |
| 终端 | — | `terminal-2` |
| 收藏 | `Star` | `star` / `star-off` |
| 更多 | `Ellipsis` | `dots` |
| 排序 | `Sort*` | `sort-ascending` / `sort-descending` |
| 分组 | `ChevronsUpDown` | `arrows-sort` |
| 隐藏文件 | `Eye` / `EyeOff` | `eye` / `eye-off` |
| 驱动器 | `HardDrive` | `device-desktop` |
| 网络 | `Globe` | `network` / `cloud` |
| 最近 | — | `clock` / `history` |
| 日历分组 | `calendar` | `calendar` |
| 窗口最小化/最大化/关闭 | Lucide `window-*` | `minus` / `square` / `x` |
| 面包屑分隔 | `ChevronRight` | `chevron-right` |

### 扩展名图标映射（`ic_*` → Tabler `file-type-*`）

| 扩展名组 | Tabler |
|----------|--------|
| pdf | `file-type-pdf` |
| html | `file-type-html` |
| rs | `file-type-rs` 或 `brand-rust` |
| ts/tsx/js | `file-type-ts` / `file-type-js` |
| cpp/h | `file-type-cpp` |
| go | `file-type-go` 或 `brand-golang` |
| java/gradle | `file-type-java` |
| json/yml/toml | `file-type-json` / `file-code` |
| 图片 | `photo` |
| 视频 | `movie` |
| 文本 | `file-text` |
| epub | `book` |
| zip | `file-zip` |
| 默认 | `file` |

更新 `list_icon_cache.rs` 中 `extension_svg_path` 与 `named_icon_paths`，指向 `icons/tabler/...`。

---

## 分阶段实施计划

每阶段结束应满足：**功能回归无差异**、`cargo build -p files-app` 通过、主要页面肉眼可对照设计稿。

### 阶段 0 — 基础设施（优先）

**目标：** 图标与主题地基，不动布局。

| 任务 | 文件/位置 |
|------|-----------|
| 新增 `scripts/sync_tabler_icons.py` | `scripts/` |
| 批量下载映射表中的 Tabler SVG 到 `assets/icons/tabler/` | `crates/app-assets/assets/icons/tabler/` |
| `icons.rs` 增加 `tabler_icon()` / `tabler_icon_element()` | `crates/files-ui/src/icons.rs` |
| 逐步将 `toolbar_icon(IconName::X)` 改为显式 Tabler 路径 | `files-ui` 全 crate |
| 新增主题集 `cyberfiles.json`（亮色/暗色，映射 gpui-component token） | `crates/app-assets/themes/` |
| 注册主题并设为新安装默认（可选，不强制覆盖用户已选主题） | `crates/app-ui/src/theme/mod.rs` |
| 更新 `app-assets` 测试：Tabler 图标可加载 | `crates/app-assets/src/lib.rs` |

**验收：** 所有工具栏/菜单/侧栏图标均为 Tabler；字体仍为 IBM Plex Sans。

---

### 阶段 1 — 标题栏 + 导航栏

**目标：** 顶部/chrome 区域视觉对齐 V11。

| 任务 | 参考 CSS | 代码位置 |
|------|----------|----------|
| 标题栏高度 46px、底部分隔线 | `.titlebar` | `app-ui/src/title_bar.rs`, `main_page/render.rs` |
| Logo + 「CyberFiles」文字（保留应用菜单） | `.app-logo` | `main_page/render.rs` |
| 标签页圆角、激活态、首页无关闭钮 | `.tb-tab`, V11 close 对齐 | `app-ui/src/tab/tab_bar.rs`, `main_page/render.rs` |
| 新建标签圆形按钮 | `.tb-tab-add` | `main_page/render.rs` |
| 主题按钮胶囊样式 | `.theme-btn` | `main_page/render.rs` |
| 导航栏 52px、按钮无边框圆角 10px | `.navbar`, `.nav-btn` | `main_page/render.rs` |
| 面包屑圆角容器 12px | `.breadcrumb` | `omnibar/` 相关渲染 |
| 分栏/固定按钮 `path-tool-btn` 样式 | `.path-actions` | `main_page/render.rs` |

**验收：** 首页隐藏导航栏；浏览目录时显示；所有按钮 action 不变。

---

### 阶段 2 — 操作栏重组（仅视觉与布局）

**目标：** 对齐设计稿 `action-bar`，不删减任何命令。

| 任务 | 说明 | 代码位置 |
|------|------|----------|
| 抽出/统一「操作栏」容器 48px | 新建/视图组/剪贴板/删除/排序 | `file_browser/render.rs` |
| 「新建」主按钮样式（绿色 primary） | 对应现有新建文件夹/文件入口 | 同上 |
| 视图模式 **分段按钮组** | 5 种视图保持，仅改变呈现 | 同上 + `render_views/` |
| 排序下拉视觉 | `.sort-area select` 圆角 10px | `file_browser/helpers.rs` 菜单样式 |
| 从内容区移除重复的视图切换条（若已上移到操作栏） | 逻辑合并到一处渲染 | `render_content_toolbar` |

**验收：** 所有原工具栏 action、快捷键、上下文菜单仍可触发。

---

### 阶段 3 — 侧栏

| 任务 | 代码位置 |
|------|----------|
| 默认宽度 214px（resizable 范围可微调） | `main_page/render_shell.rs` |
| 分组标题字间距/颜色 | `sidebar/view.rs` |
| 条目高度 35px、圆角 10px | `sidebar/` |
| 激活态：白底 + 左 3px 竖条 | `sidebar/menu_with_drop.rs` 等 |
| 驱动器使用率 **圆环**（替代或补充文字） | `sidebar/view.rs` + 小 SVG 组件 |
| 文件标签行 Tabler `tag` + 色点 | `sidebar/view.rs` |

---

### 阶段 4 — 文件列表与多视图

| 视图 | 设计要点 | 代码位置 |
|------|----------|----------|
| 详情列表 | 行高 32px、列宽、左选中条、行尾 `dots` | `render_views/table_list.rs` |
| 网格 | 118×104 卡片墙 | `render_views/tiles.rs` 或 grid |
| 卡片 | 280px 最小宽详情卡片 | `render_views/` cards |
| 多列 | 列宽 240px + 预览列 | `render_views/columns.rs` |
| 文件类型色块 | `fi.folder` / `fi.video` 等背景色 | `file_browser/helpers.rs` |
| Tabler 文件图标 | 替换 Shell 列表图标 | `helpers.rs`, `list_icon_cache.rs` |

---

### 阶段 5 — 分栏模式外壳

| 任务 | 代码位置 |
|------|----------|
| 分栏容器圆角 14px、激活窗格顶条 3px | `shell/shell_panes.rs`, `shell/pane.rs` |
| 每窗格内工具栏 44px（图标仅 Tabler） | `file_browser/render.rs` |
| 窗格标题条 34px | shell 渲染 |

---

### 阶段 6 — 首页

| 区块 | 设计要点 | 代码位置 |
|------|----------|----------|
| 快速访问 | `qa-grid` 卡片 68px 高 | `home/widgets.rs` |
| 驱动器 | 进度条 4px、激活边框 | 同上 |
| 文件标签 | 4 列 `tag-col` | 同上 |
| 最近文件 | 表格行高 34px | 同上 |
| 网络提示 | `net-notice` 信息条 | 同上 |

---

### 阶段 7 — 右键菜单与信息面板样式

| 任务 | 代码位置 |
|------|----------|
| Popup 圆角、padding、行高、hover 色 | `app-ui/src/popup_menu/popup_menu.rs` |
| 子菜单间距与毛玻璃感（在 GPUI 能力范围内近似） | 同上 |
| 信息面板边框/标题/标签页视觉统一 | `info_pane.rs` |

---

### 阶段 8 — 清理

| 任务 |
|------|
| 删除未引用的 Material / `ic_*` SVG |
| 废弃 `sync_material_icons.py` 或改为调用 Tabler 脚本 |
| 更新 `app-assets` 文档注释 |
| 设计稿与实现差异清单收尾 |

---

## 建议排期

```
阶段 0（图标+主题） → 阶段 1（标题/导航） → 阶段 2（操作栏）
        ↓
阶段 3（侧栏） → 阶段 4（列表/视图） → 阶段 5（分栏）
        ↓
阶段 6（首页） → 阶段 7（菜单/信息面板） → 阶段 8（清理）
```

阶段 0 与阶段 1 可并行部分工作，但 **图标基础设施应先于** 大规模改布局，避免重复改路径。

---

## 验收清单（每阶段通用）

- [ ] 未改动 `files-commands` action 注册与快捷键绑定
- [ ] 未改动 `file_browser/actions.rs` 业务处理逻辑（仅 `render*.rs` 布局）
- [ ] 字体仍为 IBM Plex Sans / Lilex
- [ ] 所见图标均为 Tabler outline（窗口控制、工具栏、侧栏、菜单、列表类型图标）
- [ ] 亮色/暗色主题切换正常
- [ ] `cargo build -p files-app` 与手动冒烟：首页、浏览、分栏、回收站、右键菜单、设置

---

## 相关文件索引

| 类别 | 路径 |
|------|------|
| 设计稿 | `design/cyber_files.html` |
| 主题 | `crates/app-assets/themes/`, `crates/app-ui/src/theme/mod.rs` |
| 图标资源 | `crates/app-assets/assets/icons/tabler/` |
| 图标 API | `crates/files-ui/src/icons.rs` |
| 标题栏/导航 | `crates/files-ui/src/main_page/render.rs` |
| 操作栏/列表 | `crates/files-ui/src/file_browser/render.rs`, `render_views/` |
| 侧栏 | `crates/files-ui/src/sidebar/` |
| 首页 | `crates/files-ui/src/home/` |
| 右键菜单 | `crates/app-ui/src/popup_menu/` |
| 功能路线图（业务，非 UI） | `docs/cyberfiles-feature-roadmap.md` |

---

## 修订记录

| 日期 | 说明 |
|------|------|
| 2026-06-05 | 初版：基于 `cyber_files.html` V11；约束：不改字体、不改功能、图标全 Tabler |
