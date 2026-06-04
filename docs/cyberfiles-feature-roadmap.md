# CyberFiles 复刻 Files 功能落地实现文档

本文针对 CyberFiles 相对 [Files](https://github.com/files-community/Files)（参考实现：`../Files`）尚缺的能力，给出**逐步可执行**的实现方案。每一节标注真实文件、数据结构、配置项、i18n key、边界情况与测试要点，目标是照着做即可落地。

> 说明：原「4. Undo/Redo」与「6. 撤销/重做」为同一功能，已合并为第 4 节。本文现共 **19 个功能域**（§1–§8 为核心文件体验；§9–§18 为扩展能力；§19 为双栏）。

---

## 总排期（三阶段）

### Phase A — 核心文件体验（优先推进）

| 序 | § | 功能 | 依赖 | 状态 |
|----|---|------|------|------|
| A0 | 6 | 路径自动补全 UI | 无（后端已有） | **已完成** |
| A0 | 7 | 修 shortcuts 一致性 | 无 | **已完成** |
| A1 | 1 | 归档解压 | 无 | **已完成**（含 in-process `7z.dll`） |
| A1 | 2 | Group by | 无 | **已完成**（Details/List；Grid/Cards/Columns 暂不分组） |
| A1 | 3 | 回收站 还原/清空 | platform-windows COM | **已完成** |
| A2 | 4 | Undo/Redo 文件操作 | 统一写操作入口 | **已完成** |
| A2 | 5 | 文件标签（阶段 A） | §2 可选 | **已完成** |
| A2 | 7 | 快捷键自定义 | A0 | **已完成** |
| A3 | 8 | OS↔资源管理器 拖拽 | platform-windows OLE | **8A 已完成** / 8B 待做 |

### Phase B — 搜索 / 预览 / 高级文件操作

| 序 | § | 功能 | 依赖 | 状态 |
|----|---|------|------|------|
| B1 | 9 | 全局搜索（AQS / Tag / 结果页） | §5 Tag 基础 | **已完成**（Omnibar + 结果页 + 搜索历史 + StatusCenter + Windows AQS） |
| B2 | 10 | Info pane / 预览增强 | 无 | 待做 |
| B2 | 11 | 完整属性 + 外部预览 | platform-windows | 待做 |
| B3 | 12 | Bulk rename / Flatten / Zip 浏览 / 7z 压缩 | §1 归档基础 | 待做 |

### Phase C — 平台与远程集成

| 序 | § | 功能 | 依赖 | 状态 |
|----|---|------|------|------|
| C1 | 13 | 默认文件管理器 / 打开对话框 / Jump List / 开机启动 | 注册表 + 辅助 EXE | 待做 |
| C2 | 14 | Git 集成 | 可选 libgit2 | 待做 |
| C3 | 15 | FTP / MTP 等远程位置 | 新 storage 抽象 | 待做 |

### 当前 Sprint（下一步）

1. **§19 双栏（Dual Pane）** — 对照 `../Files` 的 `ShellPanesPage`（见下文 D0–D4 子阶段）；**已完成**。
2. **§8B 拖出到资源管理器** — OLE `DoDragDrop` / `CF_HDROP`（实验性）。
3. **§10 Info pane / 预览增强**。
4. 可选：Grid/Cards 视图的分组头；§1 的 Extract… 对话框与加密包密码。

### §19 双栏 — 子排期（对照 Files）

| 序 | 阶段 | 内容 | 依赖 | 状态 |
|----|------|------|------|------|
| D0 | 基础稳固 | 双路径会话、`session_tabs` 存主栏路径、活跃窗格与 Chrome 一致、非活跃栏清选择、工具栏开/关态 | 无 | **已完成** |
| D1 | 命令与入口 | `ToggleDualPane` / `FocusOtherPane` / `CloseActivePane` / `OpenInNewPane` / `SplitPane*` 进 `files-commands`；默认快捷键 | D0 | **已完成**（菜单/设置项留 D4） |
| D2 | 可拖分栏 | 自定义 splitter（禁止第三层 `h_resizable` 嵌套）；`split_ratio` + 横/竖 `arrangement` 持久化；双击均分、拖窄关副栏 | D0 | **已完成** |
| D3 | 窄窗自适应 | 宽度 ≤750px 收起副栏并记住路径，拉宽恢复 | D2 | **已完成** |
| D4 | 增强 | 设置项（默认双栏、分栏方向、显示「在新窗格打开」）；查看菜单分栏子菜单 | D1 | **已完成** |

**Files 参考路径（`../Files`）：**

- `src/Files.App/Views/ShellPanesPage.xaml[.cs]` — 双栏容器 + `GridSplitter`
- `src/Files.App/Data/EventArguments/PaneNavigationArguments.cs` — 左右路径序列化
- `src/Files.App/Actions/Show/ToggleDualPaneAction.cs` — Ctrl+Shift+S
- `src/Files.App/Actions/Navigation/SplitPane*.cs`、`FocusOtherPane.cs`、`CloseActivePaneAction.cs`

**CyberDesktop 实现路径：**

| 区域 | 文件 |
|------|------|
| 布局 | `crates/files-ui/src/shell/shell_panes.rs` |
| 会话 | `crates/files-ui/src/main_page/session.rs`、`crates/files-core/src/config.rs`（`SessionPaneLayout`） |
| 命令 | `crates/files-commands/src/lib.rs`、`action_specs.rs` |
| 外层 layout | `crates/files-ui/src/main_page/render_shell.rs`（避免第三层 resizable 栈溢出） |
| i18n | `crates/app-ui/locales/app.yml` |

**已知约束：** 主界面已有 `main-layout` + `main-with-info-pane` 两层 `files-ui` resizable；双栏内不可再嵌套 `gpui_component::h_resizable`（会 stack overflow），D2 用轻量 splitter 或扁平 layout。

通用约定：
- 所有用户可见文案走 `rust_i18n::t!`，并在 `crates/files-app-ui/locales/app.yml`（含 `en`/`zh-CN`/`zh-Hant`）补 key。
- 纯逻辑放 `crates/files-fs`（可单测，不依赖 GPUI）；Windows 专有能力放 `crates/files-app-platform-windows`；UI 编排放 `crates/files-app-ui`。
- 动作（action）统一在 `crates/files-commands/src/lib.rs` 的 `actions!` 注册，键位在 `file_browser_key_bindings()`，在 `crates/files-app-ui/src/file_browser/render.rs` 的 `.on_action(...)` 链中挂处理函数（处理函数写在 `crates/files-app-ui/src/file_browser/actions.rs`）。
- 后台耗时操作仿照 `crates/files-app-ui/src/file_ops.rs` 的 `spawn_*` 模式：`cx.spawn` + `background_spawn` + `TransferStatusGlobal` 进度 + `Notification`。

---

## 1. 归档解压（ZIP / 7z / tar）

### 现状
- `crates/files-fs/src/archive.rs` 仅实现 ZIP **压缩**（`compress_paths_to_zip*`），无解压。
- UI 入口 `CompressItems` action（`crates/files-commands/src/lib.rs`），`compress_items()`（`crates/files-app-ui/src/file_browser/ops.rs`）+ `spawn_compress()`（`crates/files-app-ui/src/file_ops.rs`）。
- 上下文菜单可见性开关：`context_menu_show_compress`（`crates/files-core/src/config.rs`）。

### 目标
对选中的 `.zip` / `.7z` / `.tar` / `.tar.gz` / `.tgz` 提供三种解压（对齐 Files）：
- **Extract here**：解压到当前目录。
- **Extract to <名字>\\**：解压到与压缩包同名子文件夹。
- **Extract...**（可后置）：弹目标选择对话框。

### 依赖（Cargo）
在 `crates/files-fs/Cargo.toml` 增加：
```toml
sevenz-rust = "0.6"     # 7z 解压（纯 Rust）
tar = "0.4"             # tar
flate2 = "1"            # gzip (.tar.gz/.tgz)
```
`zip` crate 已在依赖中（用于读取 ZIP）。

### 步骤

**(1) 在 `crates/files-fs/src/archive.rs` 增加解压 API。**

新增格式探测与统一入口：
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat { Zip, SevenZip, Tar, TarGz }

/// 依扩展名识别，无法识别返回 None（菜单项据此显隐）。
pub fn detect_archive_format(path: &Path) -> Option<ArchiveFormat>;

pub fn is_archive_path(path: &Path) -> bool { detect_archive_format(path).is_some() }

/// 解压 `archive` 到 `dest_dir`（dest_dir 必须已存在）。
/// 报告 on_progress(completed_entries, total_entries)，可被 cancel 取消。
pub fn extract_archive_cancellable(
    archive: &Path,
    dest_dir: &Path,
    cancel: &AtomicBool,
    on_progress: impl FnMut(u32, u32),
) -> anyhow::Result<()>;
```
实现要点：
- ZIP：用 `zip::ZipArchive::new(File::open)`，遍历 `by_index`，`enclosed_name()` 防目录穿越（拒绝 `..` / 绝对路径），逐项写出，每项后回调进度，每项前查 `cancel`。
- 7z：`sevenz_rust::decompress_file` 一次性解压；若要进度则用 `SevenZReader` 自己遍历。第一版可不报细粒度进度。
- tar：`tar::Archive::new(File).unpack(dest_dir)`；`.tar.gz/.tgz` 先包一层 `flate2::read::GzDecoder`。
- 复用 `CompressCancelled` 同款错误类型，新增 `ExtractCancelled`（或复用统一 `OperationCancelled`）。

**(2) 目标目录解析辅助**（放 `archive.rs`，仿 `unique_zip_path`）：
```rust
/// "Extract to" 的子文件夹路径：与压缩包同名（去扩展名），冲突则加 (2)(3)...
pub fn extract_to_child_dir(archive: &Path) -> PathBuf;  // 注意 foo.tar.gz -> foo
```
处理 `.tar.gz`/`.tgz` 的双扩展名去除。

**(3) 新增 action。** 在 `crates/files-commands/src/lib.rs` 的 `actions!` 里加 `ExtractHere`、`ExtractToFolder`（`Extract` 对话框版可后置）。无需默认键位。

**(4) UI 编排。** 在 `crates/files-app-ui/src/file_browser/ops.rs` 新增：
```rust
pub(super) fn extract_selection(&mut self, to_subfolder: bool, window: &mut Window, cx: &mut Context<Self>);
```
- 取 `selected_paths_vec()` 中 `is_archive_path` 为真的项（多选支持）。
- 目标目录：`to_subfolder` 时 `extract_to_child_dir`，否则 `operation_directory()`。
- 调新写的 `spawn_extract(...)`。

在 `crates/files-app-ui/src/file_ops.rs` 新增 `spawn_extract`，**完全照搬 `spawn_compress` 结构**：开 `TransferStatusGlobal::begin`、后台线程跑 `extract_archive_cancellable`、`mpsc` 回传进度、结束 `end/cancel/fail` + `Notification`，最后若 `browser.shows_directory(dest)` 则 `reload()`。

**(5) 处理函数与挂载。** `crates/files-app-ui/src/file_browser/actions.rs` 加 `on_extract_here` / `on_extract_to_folder`；`render.rs` 的 `.on_action` 链补两行。

**(6) 上下文菜单项。** 在 `crates/files-app-ui/src/file_browser/context_menu.rs` 选中项菜单里，当 `selected` 全部/部分为压缩包时插入「Extract here」「Extract to <name>」（参考现有 Compress 项的插入方式与 `context_menu_show_compress` 开关；可新增 `context_menu_show_extract`，默认 true）。

**(7) i18n。** `files.extract.here` / `files.extract.to_folder` / `files.extract.extracting`（count）/ `files.extract.done` / `files.extract.failed` / `files.transfer.cancelled`（已存在）。

### 边界情况
- 目录穿越攻击（zip slip）：必须用 `enclosed_name()`/校验解析后路径仍在 `dest_dir` 内。
- 加密压缩包：`zip`/`sevenz-rust` 会报错，捕获后提示「需要密码」（密码对话框可后置）。
- 同名冲突：第一版「Extract here」直接覆盖或跳过既有文件即可；进阶可复用 `ConflictResolution` 流程。
- 空压缩包 / 损坏文件：返回 `Err`，UI 提示失败。

### 测试
- `crates/files-fs/src/archive.rs` 加 `#[cfg(test)]`：构造临时 zip（用现有压缩 API 造）→ 解压 → 校验文件树一致；测试 `extract_to_child_dir` 对 `a.zip`/`a.tar.gz` 的命名；测试 zip slip 被拒绝。

---

## 2. 分组（Group by）

### 现状
- 只有排序：`crates/files-fs/src/sort.rs`（`SortOption`/`SortDirection`/`SortPreferences`/`sort_items`）。
- 列表渲染：`display_items: Vec<FileItem>` 是平铺列表，经 `apply_filter()` 生成（`crates/files-app-ui/src/file_browser/navigation.rs`）。
- 视图渲染：`crates/files-app-ui/src/file_browser/render_views/table_list.rs`、`tiles.rs`、`columns.rs`，用 `v_virtual_list` + `item_sizes`。

### 目标
对齐 Files 的 Group by：None / Name(首字母) / DateModified / DateCreated / Size(分桶) / Type / Tag。分组后在列表中插入**分组标题行**，标题可折叠，组内仍按当前排序。

### 设计要点
分组本质是：在 `display_items` 之上再生成「带分组头的渲染序列」。推荐引入一个渲染行枚举，避免大改虚拟列表：

**(1) `crates/files-fs/src/sort.rs`（或新文件 `group.rs`）增加：**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupOption { None, Name, DateModified, DateCreated, Size, FileType, Tag }

/// 返回每个 item 所属分组的 (key, 可显示标题)。key 用于稳定排序/折叠状态。
pub fn group_key_for(item: &FileItem, option: GroupOption) -> (String, String);
```
- Name：首字符大写（非字母归入 `#`）。
- 日期：分桶为 Today / Yesterday / Earlier this week / Last week / Earlier this month / Older（可先粗分：今天/本周/本月/更早）。
- Size：Empty(0) / Tiny(<16KB) / Small(<1MB) / Medium(<128MB) / Large(<1GB) / Huge。
- Type：用 `extension`（无扩展名归「File」/「Folder」）。
- Tag：需要 item→tags 映射（见第 5 节）；第一版可仅在 tag 系统就绪后开放。

**(2) 生成分组渲染序列。** 在 `crates/files-fs/src/sort.rs` 增加：
```rust
pub struct FileGroup { pub key: String, pub title: String, pub item_indices: Vec<usize> }

/// 在已排序的 items 上分组；组的先后顺序遵循 direction（日期/大小按桶序，名称/类型按字典序）。
pub fn group_items(items: &[FileItem], group: GroupOption, dir: SortDirection) -> Vec<FileGroup>;
```

**(3) FileBrowser 状态。** `crates/files-app-ui/src/file_browser.rs` 的 `FileBrowser` 增加：
```rust
group_option: GroupOption,
collapsed_groups: BTreeSet<String>,   // 折叠的分组 key
```
并定义渲染行：
```rust
enum DisplayRow { GroupHeader { key: String, title: String, count: usize, collapsed: bool }, Item(usize) }
```
在 `apply_filter()` 末尾，若 `group_option != None`，根据 `group_items` + `collapsed_groups` 计算出 `display_rows: Vec<DisplayRow>`；`item_sizes` 改为按 `display_rows` 生成（分组头一个固定高度，item 用原高度）。`None` 时退化为现有逻辑（每行都是 `Item`）。

**(4) 渲染。** 在 `table_list.rs` / `tiles.rs` 的虚拟列表项渲染里，按 `DisplayRow` 分支：`GroupHeader` 渲染一个带 chevron 的标题行（点击切换 `collapsed_groups` 并重算 `apply_filter`）；`Item(i)` 走现有渲染。Columns 视图（`columns.rs`）可第一版不支持分组（Files 的 ColumnView 同样不分组）。

**(5) Action + 持久化。** 
- `crates/files-commands/src/lib.rs` 加 `GroupByNone/Name/Modified/Created/Size/Type/Tag` actions。
- `actions.rs` 加 `on_group_by_*` → 设 `group_option`、`apply_filter()`、`persist_prefs()`、`cx.notify()`。
- `render.rs` 挂 `.on_action`。
- 持久化：`crates/files-core/src/config.rs` 的 `AppConfig` 加 `#[serde(default)] pub file_group_option: Option<String>`，扩展 `save_file_browser_prefs(...)` 签名（当前是 5 参，加第 6 参 group），并加常量 `GROUP_NONE/NAME/...` 与 `file_group_from_config()`。`FileBrowser::with_options` 启动时读取。
- 工具栏/右键菜单：在现有 Sort 菜单旁加 "Group by" 子菜单（`context_menu.rs` 与 `render.rs` 的工具栏下拉）。

### 边界情况
- 分组 + 过滤（搜索）同时生效：先过滤得 `display_items`，再分组。
- 「目录优先」`directories_first`：分组后仍应保证文件夹组靠前（Name/Type 分组时文件夹可单独成「Folders」组或并入字母组——建议 Files 行为：仍受 directories_first 影响，文件夹排在各自分组顶部；最简实现先按 key 分组，组内 `sort_items` 已处理目录优先）。
- 折叠状态在导航到新目录时清空（`navigate_to` 里重置 `collapsed_groups`）。

### 测试
- `group_items` 单测：日期/大小分桶边界、Name 非字母归 `#`、空目录、direction 反转组顺序。

---

## 3. 回收站 还原 / 清空 专用 action

### 现状
- 浏览：`BrowseLocation::RecycleBin`、`read_recycle_bin()`（`crates/files-fs/src/recycle.rs`）→ `list_recycle_bin_entries()`（`crates/files-app-platform-windows/src/recycle.rs`，走 Shell namespace）。
- `RecycleBinEntry { display_name, shell_path, size, modified }`，`shell_path` 是解析路径，可用于 Shell verbs。
- 删除到回收站：`recycle_paths()`（`crates/files-fs/src/ops.rs`，用 `trash` crate）。
- **缺**：还原单项 / 还原全部 / 清空回收站。

### 目标
- **Restore**：对回收站中选中项执行 Shell `restore` 动词，还原到原位置。
- **Restore all**：对所有项执行 restore。
- **Empty Recycle Bin**：清空（带确认）。

### 步骤

**(1) platform-windows 增加 COM 能力。** 新增 `crates/files-app-platform-windows/src/recycle.rs` 中函数（或新文件 `recycle_ops.rs`）：
```rust
/// 清空回收站（带 Win32 进度 UI）。
pub fn empty_recycle_bin() -> anyhow::Result<()>;   // SHEmptyRecycleBinW(None, None, 0)

/// 还原指定回收站项（按 shell_path 解析 PIDL 后执行 "restore"/"undelete" 动词）。
pub fn restore_recycle_bin_items(shell_paths: &[PathBuf]) -> anyhow::Result<()>;
```
实现：
- `empty_recycle_bin`：直接调用 `windows::Win32::UI::Shell::SHEmptyRecycleBinW`（flags 可用 `SHERB_NOCONFIRMATION` 因为我们自己弹确认，或不带让系统确认）。
- `restore_recycle_bin_items`：枚举回收站 `IShellFolder`（复用 `list_recycle_bin_entries_inner` 的绑定逻辑），对匹配 `shell_path` 的 PIDL 用 `IContextMenu`（`GetUIObjectOf` 取 `IID_IContextMenu`）查找并 invoke `restore`（旧称 `undelete`）动词。这是与 `crates/files-app-platform-windows/src/context_menu.rs`/`shell.rs` 同一套 `IContextMenu::InvokeCommand` 模式，可复用其 verb 调用封装。
- 在 `crates/files-app-platform-windows/src/lib.rs` `pub use` 导出这两个函数，并在 `mod stubs` 里加非 Windows 空实现。

**(2) fs 层薄封装**（可选，便于跨平台/测试）：`crates/files-fs/src/recycle.rs` 加：
```rust
#[cfg(windows)] pub fn empty_recycle_bin() -> anyhow::Result<()> { app_platform_windows::empty_recycle_bin() }
#[cfg(windows)] pub fn restore_recycle_items(paths: &[PathBuf]) -> anyhow::Result<()> { ... }
// 非 windows 返回 Ok(())/bail
```

**(3) Actions。** `crates/files-commands/src/lib.rs` 加 `RestoreRecycleItems`、`RestoreAllRecycleItems`、`EmptyRecycleBin`。

**(4) 处理函数。** `crates/files-app-ui/src/file_browser/actions.rs`：
- `on_restore_recycle_items`：仅当 `browse_location == RecycleBin`；取选中项的 `path`（即 `shell_path`）→ 后台 `restore_recycle_bin_items` → `refresh()` + 成功通知。
- `on_restore_all`：用 `self.items` 全部 `path`。
- `on_empty_recycle_bin`：`window.open_alert_dialog`（参考 `ops.rs::confirm_delete_inner` 的弹窗写法）确认后后台 `empty_recycle_bin` → `refresh()`。
- `render.rs` 挂 `.on_action`。

**(5) UI 入口。**
- 回收站工具栏：当 `browse_location == RecycleBin` 时，在内容工具栏（`render.rs`）显示「Restore all」「Empty Recycle Bin」按钮（参考现有工具栏按钮 `toolbar_labeled_button`）。
- 右键菜单（`context_menu.rs`）：回收站项上显示「Restore」「Delete(permanent)」；空白处显示「Empty Recycle Bin」。需要在 `context_menu.rs` 里根据 `self.browse_location` 分支构造不同菜单（目前主要面向 `Directory`）。

**(6) i18n。** `files.recycle.restore` / `restore_all` / `empty` / `empty.confirm` / `empty.title` / `restore.success` / `empty.success` 等。

### 边界情况
- 还原时原目录已不存在：Shell 会重建目录或报错，捕获并提示。
- 多选还原：逐项 invoke；部分失败时汇总提示。
- 清空时回收站为空：`SHEmptyRecycleBinW` 仍返回成功，UI 照常 refresh。
- COM 公寓：调用前 `ensure_com_apartment()`（已有，见 `recycle.rs`）。

### 测试
- COM 行为难单测；至少为 fs 封装写编译期/非 windows 桩测试。手动验证：删除文件→进回收站→Restore→回原位；Empty 后列表空。

---

## 4. Undo / Redo 文件操作历史

### 现状
- 文件操作分散：`copy_items/cut_items/paste_items`（`ops.rs`）、`spawn_file_transfer`/`spawn_paste_from_clipboard`（`file_ops.rs`）、`rename_path`（`crates/files-fs/src/ops.rs` + `rename.rs`）、`create_directory/create_file`、`recycle_paths/delete_paths`、`create_folder_from_selection`。
- 无任何撤销栈。

### 目标
对齐 Files `StorageHistory`：记录可逆操作并支持 `Ctrl+Z` / `Ctrl+Y` 撤销重做。可逆操作及其逆操作：

| 操作 | 记录内容 | 撤销 | 重做 |
|------|----------|------|------|
| Move/Paste(剪切) | 每个 `(src, dst)` | 把 dst 移回 src | 再次 src→dst |
| Copy/Paste(复制) | 新产生的 `dst` 列表 | 删除 dst（到回收站） | 再次复制 |
| Rename | `(old, new)` | new→old | old→new |
| New folder/file | 新建 `path` | 删除 path | 重新创建 |
| Recycle(删除到回收站) | 原 `paths` | 从回收站还原（依赖第 3 节 restore） | 再次 recycle |
| 永久删除 | — | **不可撤销**（不入栈） |

### 设计

**(1) fs 层定义历史模型。** 新建 `crates/files-fs/src/history.rs`：
```rust
#[derive(Debug, Clone)]
pub enum FileOperation {
    Move { moves: Vec<(PathBuf, PathBuf)> },         // (from, to)
    Copy { created: Vec<PathBuf> },                  // 复制产生的目标
    Rename { from: PathBuf, to: PathBuf },
    Create { path: PathBuf },
    Recycle { originals: Vec<PathBuf> },
}

#[derive(Default)]
pub struct OperationHistory { undo: Vec<FileOperation>, redo: Vec<FileOperation> }

impl OperationHistory {
    pub fn record(&mut self, op: FileOperation);     // push undo, clear redo
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
    pub fn take_undo(&mut self) -> Option<FileOperation>;
    pub fn take_redo(&mut self) -> Option<FileOperation>;
    pub fn push_redo(&mut self, op: FileOperation);
    pub fn push_undo(&mut self, op: FileOperation);  // 重做后回填
}
```
并提供纯函数 `undo_operation(&FileOperation) -> anyhow::Result<FileOperation>`（返回执行后的「反向操作」用于 redo 回填）与 `redo_operation(...)`。底层复用 `crate::clipboard::transfer_one` / `crate::ops::*` / `crate::recycle`。

**(2) 全局历史存放。** 历史是「每窗口」还是「全局」？Files 是每窗口。最简：放进程级 `App` global（仿 `AppFileClipboard`，见 `crates/files-app-ui/src/app_state.rs`）。新增 `AppOperationHistory`，提供 `record/undo/redo/can_*` 静态方法（内部 `cx.global_mut`）。

**(3) 统一记录点。** 关键改造——所有写操作完成后调用 `AppOperationHistory::record`：
- `spawn_file_transfer` / `spawn_paste_from_clipboard`（`file_ops.rs`）：在 `run_transfer_with_conflicts` 成功路径收集实际发生的 `(src,dst)`（move）或 `dst`（copy），完成后 record。注意只记录真正 transferred 的项（跳过 skip 的）。
- `rename`（`rename.rs` 的 commit 处）：record `Rename`。
- `create_new_folder/create_new_file`（`ops.rs`）：record `Create`。
- 删除（`confirm_delete_inner` 的 recycle 分支）：record `Recycle`；**永久删除分支不 record**。
- `create_folder_from_selection`：本质是 Create + Move，可记录为复合（简化：记录 Move，新文件夹的删除留给手动）。第一版可只记录 Move 部分。

**(4) Actions + 键位。**
- `crates/files-commands/src/lib.rs` 加 `UndoOperation`、`RedoOperation`；键位 `ctrl-z` / `ctrl-y`（mac `cmd-z`/`cmd-shift-z`）。注意：当前没有全局文本输入抢占 Ctrl+Z 的问题需确认（重命名 InputState 激活时应让输入框优先——在 `on_undo` 里判断 `self.renaming.is_some()` 则 `cx.propagate()`）。
- `actions.rs` 加 `on_undo`/`on_redo`：调用 `AppOperationHistory::undo/redo`（后台执行，仿 spawn 模式），完成后 `refresh()` + 通知 `t!("files.undo.done")`。
- `render.rs` 挂 `.on_action`。

**(5) 工具栏按钮（可选）。** 在内容工具栏加 Undo/Redo 图标按钮，`disabled` 绑定 `AppOperationHistory::can_undo/redo`。

### 边界情况
- 撤销 Copy（删除 dst）：删除前确认 dst 仍是当时复制出来的内容（路径存在即删；可不做内容校验，第一版到回收站而非永久删，更安全）。
- 撤销 Move 时目标位置被占：走 `ConflictResolution` 或直接报错并保留历史项。
- 外部修改导致 src/dst 已不存在：撤销失败时弹错误，并**丢弃**该历史项（不要无限卡住）。
- 历史栈上限（如 50 条）防内存膨胀。
- 历史**不持久化**（重启清空），与 Files 一致。

### 测试
- `history.rs` 纯逻辑单测：在 tempdir 跑 move→undo→redo、rename→undo、create→undo，校验文件系统状态与 redo 栈回填正确。

---

## 5. 文件标签系统

### 现状
- 配置：`FileTagConfig { name, color: Option<String>, paths: Vec<String> }`（`crates/files-core/src/config.rs`），`AppConfig.file_tags`。注释明确「full tag system is future work」。
- 浏览：`BrowseLocation::FileTag { tag_name }` → `paths_for_file_tag` → `file_items_for_tag_paths`（`crates/files-fs/src/file_tag.rs`）。
- 赋值/移除：`assign_paths_to_file_tag` / `remove_paths_from_file_tag`（`crates/files-app-ui/src/shell/preferences.rs`），入口在 `context_menu.rs`（受 `context_menu_show_file_tags` 控制）。
- 设置页：`settings_view.rs` 可增删标签名、显示 path 计数。
- **缺**：标签颜色设置 UI、列表中标签视觉标记、按标签分组/排序、一个文件多标签的便捷管理、（可选）写入 NTFS 流。

### 目标（分阶段）
**阶段 A（纯配置增强，推荐先做）**
1. 标签颜色选择 UI。
2. 文件行/卡片上显示其所属标签的彩色圆点/胶囊。
3. 右键「Tags」子菜单支持勾选多个标签（toggle），而非只赋值。
4. 排序/分组按 Tag（接第 2 节 GroupOption::Tag、第 1 节 SortByTag）。

**阶段 B（可后置）**
5. 标签写入 NTFS 备用数据流（与资源管理器/PowerToys 互通），需要 platform-windows 读写 ADS。

### 步骤（阶段 A）

**(1) 建立「路径→标签」反查。** 当前数据是「标签→路径列表」，渲染文件行时需反查。新增 `crates/files-fs/src/file_tag.rs`：
```rust
pub struct TagRef { pub name: String, pub color: Option<String> }
/// 给定一批 (tag_name, color, paths)，构造 path→Vec<TagRef> 映射。
pub fn build_path_tag_index(tags: &[(String, Option<String>, Vec<PathBuf>)]) -> HashMap<PathBuf, Vec<TagRef>>;
```
`FileBrowser` 持有 `path_tags: HashMap<PathBuf, Vec<TagRef>>`，在 `refresh()`/标签变更时从 `load_config().file_tags` 重建。

**(2) 渲染标签标记。** 在 `table_list.rs`（详情视图加一列或名字后缀）、`tiles.rs`（卡片角标）渲染 `path_tags.get(&item.path)`，用 `color` 画圆点。颜色字符串解析复用 `crates/files-app-ui/src/color_icon.rs` 的配色或主题色映射；`FileTagConfig.color` 建议存 hex（如 `#E53935`）。

**(3) 颜色选择 UI。** `settings_view.rs` 标签编辑行加一组预设色板按钮（Files 用固定调色板），点击写 `FileTagConfig.color` 并 `save_config`。预设色定义为常量数组。

**(4) 多标签 toggle 菜单。** `context_menu.rs` 的「Tags」子菜单：对每个已定义标签显示「✓/空 + 颜色点 + 名称」，点击 toggle（在该标签 paths 中增删当前选择）。复用 `assign_paths_to_file_tag`/`remove_paths_from_file_tag`，并据当前选择是否已在该标签内决定 toggle 方向。变更后重建 `path_tags`、`refresh`。

**(5) 排序/分组接入。** 
- `SortOption::Tag`：比较时用「该 item 的首个标签名」（无标签排最后）。需要 `sort_items` 能访问 `path_tags`——可改为在 UI 层分组前注入，或给 `FileItem` 增加 `tags: Vec<TagRef>` 字段（更直接）。**推荐**：在 `refresh()` 生成 `items` 后，遍历填充 `FileItem.tags`（需在 `crates/files-fs/src/item.rs` 给 `FileItem` 加 `#[serde skip]`-style 字段 `pub tags: Vec<String>`，默认空）。这样 sort/group 都能用。
- `GroupOption::Tag`、`SortByTag` action：接第 1、2 节。

**(6) i18n。** `files.tags.choose_color`、`files.tags.toggle` 等。

### 阶段 B（NTFS ADS，可后置）
- platform-windows 新增 `read_file_tags(path)`/`write_file_tags(path, &[tag_id])`：写入 `path:files.cyber_desktop.tags`（或兼容 Files 的 `:com.files.tags` 流名以互通）。用 `CreateFileW` + 流名后缀。
- 同步策略：以 ADS 为准还是 config 为准需定义；Files 用独立数据库 + ADS。第一版建议保持 config 为单一数据源，ADS 仅作为可选导出。

### 边界情况
- 删除/移动文件后，config 里残留死路径：`paths_for_file_tag` 已 `if !path.exists() continue`；可在 Undo/移动时同步更新标签路径（进阶）。
- 同一文件多标签渲染过多：最多显示 N 个点 + "+k"。

### 测试
- `build_path_tag_index` 单测；`group_key_for(Tag)` 单测。

---

## 6. 路径自动补全（Omnibar）

### 现状
- 后端**已就绪**：`omnibar_path_suggestions(query, path_history)`（`crates/files-fs/src/omnibar.rs`）返回 `Vec<OmnibarPathSuggestion>`：空输入给历史，否则给子目录匹配。
- 历史已持久化：`AppConfig.path_history`，`crates/files-core/src/path_history.rs`。
- Omnibar 路径编辑：`enter_omnibar_path_edit` / `submit_omnibar_path`（`crates/files-app-ui/src/main_page/omnibar.rs`）。
- 面包屑 chevron 下拉已有完整实现（`breadcrumb_flyout.rs`），可作为 UI 参照。
- **缺**：路径编辑态下的补全下拉 UI 未接线。

### 目标
在 Omnibar 进入路径编辑模式后，随输入实时弹出补全列表（历史 + 子目录），支持上下键选择、Enter 跳转、Esc 关闭、Tab 补全到高亮项。

### 步骤

**(1) 定位 Omnibar 输入状态。** 在 `crates/files-app-ui/src/main_page/omnibar.rs`（及承载 omnibar 输入 `InputState` 的结构，可能在 `crates/files-app-ui/src/main_page/mod.rs` 或 `omnibar.rs`）找到路径编辑用的 `InputState` 与其 `InputEvent` 订阅。

**(2) 增加补全状态。** 在 omnibar 宿主结构加：
```rust
omnibar_suggestions: Vec<OmnibarPathSuggestion>,
omnibar_suggestion_index: Option<usize>,   // 高亮项
omnibar_suggestions_open: bool,
```

**(3) 订阅输入变化。** 在已有的 `InputEvent::Change`（或等价）回调里：取当前文本 → 调 `omnibar_path_suggestions(text, &config.path_history)` → 写入 `omnibar_suggestions`、`open = !empty`、重置高亮 → `cx.notify()`。`read_options` 不需要（该函数只列目录）。为避免每键 IO 卡顿，可用 `cx.spawn` 防抖（150ms）后台 `read_dir`（`omnibar_path_suggestions` 内部有 IO）。

**(4) 渲染下拉。** 在 omnibar 渲染处，`omnibar_suggestions_open` 时用 `anchored`/`deferred`（参考 `file_browser.rs` 顶部 import 与现有 popup）的浮层，列出 `label`，高亮 `suggestion_index`，行 `on_mouse_down` → 选中并 `submit_omnibar_path(path)`。`dimmed` 项弱化显示。

**(5) 键盘导航。** 给路径输入框的按键处理（或在 omnibar 容器加 `on_key_down`）：Up/Down 移动 `suggestion_index`；Enter 若有高亮则跳转高亮项，否则提交输入框原文；Esc 关闭下拉（再次 Esc 退出编辑态）；Tab 把高亮项 `label` 写回输入框。

**(6) 提交即记历史。** `submit_omnibar_path` 成功导航后，确保把目标写入 `path_history`（若 `main_page/navigation.rs` 尚未做则补上，去重 + 上限）。

### 边界情况
- 输入 `home`/`settings`/`recycle bin` 特殊词：函数已返回历史；提交时走对应 `BrowseLocation`（`submit_omnibar_path` 应已处理）。
- Windows 盘符 `C:` vs `C:\`：`parent_and_partial` 已处理大部分；测试 `C:\Us` → 列 `C:\Users`。
- 无权限目录：`read_dir` 失败返回空，下拉不弹。
- 性能：限 `MAX_SUGGESTIONS = 10`（已有）。

### 测试
- `omnibar_path_suggestions` 已可单测（纯函数 + 临时目录）；补 UI 层手动验证导航/键盘。

---

## 7. 快捷键自定义

### 现状
- 键位**硬编码**：`file_browser_key_bindings()`（`crates/files-commands/src/lib.rs`）在 `init()` 里 `cx.bind_keys(...)`。
- 设置页「Actions」是**只读**参考表：`shortcut_reference()`（`crates/files-commands/src/shortcuts.rs`）→ `settings_view.rs` 展示。
- 文档与实际有不一致（`shortcuts.rs` 写 Ctrl+2=grid/Ctrl+3=columns，实际 `lib.rs` 是 Ctrl+2=List / Ctrl+3=Grid / Ctrl+4=Columns）——**应先修这个 bug**。

### 目标
对齐 Files `ActionsSettingsService`：每个 action 可在设置页改键位，持久化，冲突检测，可重置默认。

### 步骤

**(0) 先修文档一致性 bug。** 校正 `shortcut_reference()` 使其与 `file_browser_key_bindings()` 完全一致（含 List/Grid/Cards/Columns、ReopenTab、ToggleShowFileExtensions 等缺失项）。这是把「真相源」统一的前置工作。

**(1) 建立 action 注册表（单一真相源）。** 在 `crates/files-commands/src/lib.rs` 定义稳定的 action 标识与默认键位的统一表：
```rust
pub struct ActionSpec {
    pub id: &'static str,            // 稳定 key，如 "navigate_back"，用于配置存储
    pub default_keystroke: &'static str,  // "alt-left"
    pub context: Option<&'static str>,    // Some(FILE_BROWSER) / None
    pub i18n_key: &'static str,      // "settings.actions.navigate_back"
    pub make_binding: fn(&str) -> KeyBinding,  // 用给定 keystroke 生成对应 action 的 KeyBinding
}
pub fn action_specs() -> &'static [ActionSpec];
```
`make_binding` 针对每个 action 写一个闭包（因为 `KeyBinding::new(keystroke, ConcreteAction, ctx)` 需要具体 action 类型）。`file_browser_key_bindings()` 改为遍历 `action_specs()` 用 `default_keystroke` 生成（消除重复定义）。

**(2) 配置存储。** `AppConfig` 加：
```rust
#[serde(default)] pub keybindings: std::collections::BTreeMap<String, String>,  // action_id -> keystroke（覆盖默认）
```
加 `pub fn keybinding_overrides() -> BTreeMap<String,String>` / `save_keybinding(id, keystroke)` / `reset_keybinding(id)`。

**(3) 构建实际绑定。** 新增 `pub fn resolve_key_bindings() -> Vec<KeyBinding>`：遍历 `action_specs()`，键位取 `overrides.get(id).unwrap_or(default)`，调 `make_binding`。`init(cx)` 改为 `cx.bind_keys(resolve_key_bindings())`。

**(4) 运行时重绑定。** GPUI 支持重设键位。提供 `pub fn rebind_all(cx: &mut App)`：`cx.clear_key_bindings()` 后 `cx.bind_keys(resolve_key_bindings())`（确认当前 gpui 版本的 API 名；若无 `clear_key_bindings`，则需在改键后提示「重启生效」作为降级方案）。改键保存后调用它即时生效。

**(5) 设置页 UI。** `settings_view.rs` 的 Actions 页：每行 = i18n 名称 + 当前键位（可点击）+「录制」+「重置」。点击「录制」进入捕获模式：监听下一个组合键，转成 gpui keystroke 字符串（如 `ctrl-shift-k`），写 `save_keybinding` → `rebind_all` → 刷新。
- **冲突检测**：保存前检查同 `context` 下是否已有相同 keystroke；冲突则高亮提示（红字），可选择覆盖（清除旧的或拒绝）。
- **重置**：单条 `reset_keybinding` / 全部重置按钮。

**(6) keystroke 文本化辅助。** 写 `keystroke_to_display(&str) -> String`（`ctrl-shift-k` → `Ctrl+Shift+K`）与逆向解析，供 UI 显示与录制使用。

### 边界情况
- 与系统/输入框冲突的键（如 Ctrl+C 在重命名输入框）：保持 context=FILE_BROWSER，输入态优先。
- 无效/危险绑定（如只绑单个修饰键）：录制时校验至少含一个非修饰键。
- macOS 与 Windows 默认键差异：`ActionSpec` 可加 `default_keystroke_mac`，或在 `make_binding` 内按 `cfg!` 处理（保留现有 mac 分支思路）。
- 旧版本配置无 `keybindings` 字段：`#[serde(default)]` 兜底。

### 测试
- `resolve_key_bindings` 在有/无 override 下数量与内容正确；`keystroke_to_display` 往返；冲突检测单测。

---

## 8. OS ↔ 资源管理器 拖拽

> 最复杂，涉及 Win32 OLE 拖放。建议拆「拖入」和「拖出」两步，先做拖入（更易、价值高）。

### 现状
- 仅应用内拖拽：payload `DraggedFilePaths`（`crates/files-app-ui/src/drag.rs`，注释明确「not OS shell drag type」）。
- 应用内 drop 逻辑成熟：`handle_drop` / `handle_drop_on_item`（`crates/files-app-ui/src/file_browser/core.rs`）→ `spawn_file_transfer`。
- 已能读外部剪贴板文件列表：`read_clipboard_file_paths()`（platform-windows），`paste_items` 用作回退。

### 8A. 从资源管理器拖入 CyberFiles（Drop-in）

**(1) 确认 GPUI 外部文件 drop 能力。** GPUI/Zed 在 Windows 通过 `IDropTarget` 把拖入的文件作为 `ExternalPaths`/`DragMoveEvent<ExternalPaths>` 投递。需在当前 gpui 版本核对类型名（grep gpui crate 中 `ExternalPaths` / `FileDropEvent` / `on_drop`）。

**(2) 在文件列表容器接收外部 drop。** 在 `render.rs` 的文件列表根元素上，除现有 `on_drop::<DraggedFilePaths>` 外，再加 `on_drop::<gpui::ExternalPaths>`（或对应类型）：拿到外部路径后调用现有 `self.handle_drop(paths, window, cx)`（复用 move/copy 逻辑；外部拖入默认 Copy 更安全，按住 Shift 为 Move——与 Files/Explorer 习惯一致，注意与内部「Ctrl=Copy」区分）。
- 同样给文件夹行（`render_views/*`）、面包屑、侧边栏的 drop 目标加外部类型分支。

**(3) 视觉反馈。** 复用现有 `set_drag_hover_feedback` 路径（hover 文件夹高亮）。

### 8B. 从 CyberFiles 拖出到资源管理器（Drag-out）

这是难点：需要在拖拽开始时提供一个 OLE `IDataObject`（含 `CF_HDROP`），并用 `DoDragDrop` 驱动。GPUI 的拖拽是内部实现，不会自动暴露为 OLE 源。两条路线：

**路线一（推荐，独立 OLE 拖出）：** 在 platform-windows 实现一个「按需启动的 OLE 拖出」：
```rust
// crates/files-app-platform-windows/src/drag_out.rs
/// 阻塞式启动 OLE 拖放，提供选中文件的 CF_HDROP。allowed = Copy|Move。
/// 返回最终 effect（Copy/Move/None）。需在拥有消息循环的线程调用。
pub fn begin_drag_out(paths: &[PathBuf], allow_move: bool) -> anyhow::Result<DragEffect>;
```
实现：构造 `IDataObject`（可用 Shell 的 `SHCreateDataObject` + 子项 PIDL，或自己实现 `IDataObject` 提供 `CF_HDROP`/`CFSTR_FILEDESCRIPTOR`）+ 简单 `IDropSource`，调 `DoDragDrop(data, drop_source, DROPEFFECT_COPY|MOVE, &mut effect)`。
- 触发时机：GPUI 不直接给「原生拖拽开始」钩子，因此用**鼠标按下后移动超过阈值**自行判定（`on_mouse_down` 记录起点，`on_mouse_move` 超阈值且在文件行上 → 调 `begin_drag_out`）。注意 `DoDragDrop` 是**阻塞**调用，需在 UI 线程的消息泵环境中运行，且会接管鼠标，与 GPUI 内部拖拽**互斥**——需要一个开关：在文件行上向「外」拖时用 OLE，应用内拖拽则保留现状。第一版可用修饰键或起手即判定区分，体验需打磨。

**路线二（折中，降级体验）：** 不做真正拖出，提供「拖动时自动复制路径/文件到剪贴板」或仅支持「拖入」。先交付 8A，8B 标记为后续。

**(4) 导出与桩。** `lib.rs` 导出 `begin_drag_out`、`DragEffect`，非 Windows 桩返回 `DragEffect::None`。

### 边界情况
- Move 拖出后源文件被外部删除：拖出完成后 `refresh()` 即可反映。
- 拖出大量/大文件：`DoDragDrop` 由目标进程执行复制，本进程只提供数据，通常无需自己拷贝。
- 与应用内拖拽冲突：必须明确「何时走 OLE，何时走 GPUI 内部」，避免双触发。建议：拖向窗口外/跨进程才尝试 OLE——但 GPUI 难以预知方向，故第一版可：**8A 优先上线**，8B 作为实验开关（设置项 `experimental_native_drag_out`）。

### 测试
- 8A：从资源管理器拖文件到 CyberFiles 文件夹 → 触发 copy/move，列表刷新。
- 8B：拖文件到桌面/资源管理器窗口 → 出现文件副本。COM 部分难自动化，以手动为主。

---

## 9. 全局搜索（Windows Search / AQS、Tag 查询、搜索结果专页）

### 现状
- **Ctrl+F** 进入 Omnibar **全局搜索模式**（`search.global.placeholder`）；**Ctrl+L** 进入路径编辑。Enter 提交后在当前 tab 打开 `NavigationTarget::SearchResults` / `BrowseLocation::SearchResults`，后台 `search_folder`（Plain/Tag 递归扫描；`$` AQS 在 Windows 上走索引 COM，失败时降级递归）。
- 工具栏 `#nav-search-wrap` 仍为当前目录**列表过滤**（`filter_items_by_query`），与全局搜索分离。
- 支持 `tag:Name` 与 `$` 前缀 AQS 语法；`AppConfig.search_history` + Omnibar 下拉展示最近查询。
- 大目录搜索在 **StatusCenter** 显示可取消进度（与 `_search_cancel` 联动）。
- 搜索结果 Details/List 视图显示**路径列**；默认按路径排序；双击文件跳转所在文件夹并选中。
- Esc 退出 Omnibar 搜索模式（或先关闭搜索历史下拉）。
- `text-engine` 有 `global_search`（给 CyberEditor 用），与文件管理器搜索无关。

### 目标（对齐 Files `FolderSearch` + Omnibar Search 模式）
1. **Omnibar 搜索模式**：Ctrl+F 进入「搜索当前位置」模式（与 Ctrl+L 路径模式区分）；输入框 placeholder / 图标变化。
2. **Windows Search（AQS）**：在 Windows 上调用 `ISearchFolderItemFactory` / 索引搜索（Files 用 `FolderSearch.cs`）；支持 `$` 前缀原始 AQS；无索引时降级为递归文件名扫描（可配置超时/深度）。
3. **Tag 查询**：语法如 `tag:Work` 或 Files 兼容的 Tag 表达式；解析后走 `paths_for_file_tag` + 可选子树过滤。
4. **搜索结果专页**：新 `BrowseLocation::SearchResults { query, scope }`；独立 tab；列：名称、路径、修改时间；支持 Sort by Path；点击结果跳转所在文件夹并选中项。
5. **搜索历史**：`AppConfig.search_history: Vec<String>`，Omnibar 下拉展示最近查询。

### 步骤（概要）

**(1) fs 层。** 新建 `crates/files-fs/src/folder_search.rs`：
```rust
pub enum SearchScope { CurrentFolder, Home, Library(PathBuf), Tag(String) }
pub struct SearchHit { pub path: PathBuf, pub display_name: String, pub modified: Option<SystemTime> }
pub fn search_folder(scope: SearchScope, query: &str, cancel: &AtomicBool) -> anyhow::Result<Vec<SearchHit>>;
```
Windows 实现放 `platform-windows/src/search.rs`（COM / 索引）；非 Windows 用 `walkdir` 递归 + 文件名匹配。

**(2) Tag / AQS 解析。** `parse_search_query(query) -> SearchQuery` 区分 Plain / Aqs / Tag。

**(3) UI。** `MainPage` / `omnibar.rs`：搜索模式状态、`submit_search` → 新开 tab 或替换当前为 `SearchResults`；`FileBrowser` 增加 `load_search_results(hits)` 渲染路径列。

**(4) Actions。** `GlobalSearch`（Ctrl+F）、`ClearSearch`（Esc）；与路径编辑模式互斥。

### Files 参考
- `Files.App/Utils/Storage/Search/FolderSearch.cs`
- `NavigationToolbarViewModel`（OmnibarSearchMode）
- `ContentPageTypes.SearchResults`

### 边界情况
- 大目录递归搜索需 cancellable + 进度（StatusCenter）。
- 无 Windows Search 索引时 AQS 可能失败 → 提示并降级。
- Tag 查询与 §5 标签数据源一致。

### 测试
- `parse_search_query` 单测；tempdir 递归扫描单测；Windows COM 手动验收。

---

## 10. Info pane / 预览增强

### 现状
- `crates/files-app-ui/src/info_pane.rs`：Details + Preview 两 tab；单选时显示 path/type/size/modified。
- `crates/files-fs/src/preview.rs`：Image/SVG/Markdown/HTML/Code/Text；64KB 文本截断；**无** PDF/音视频/文件夹统计/多选摘要。
- 创建时间、属性（只读/隐藏/系统）未展示；预览标题部分硬编码英文。

### 目标
1. **多选摘要**：N items selected、总大小合计、类型分布（对齐 Files Details tab）。
2. **单选扩展字段**：Created、Accessed（Windows `GetFileAttributesEx`）、Attributes 只读/隐藏/系统/目录。
3. **更多预览类型**（分阶段）：
   - **B1**：PDF（第一页 raster 或嵌入 WebView2 / 仅显示「用系统打开」占位 + 页数元数据）
   - **B2**：音视频（统一走 `rust-ffmpeg` 后端；Video 为 GPUI + D3D11 纹理渲染，解码策略固定为**优先硬解，失败自动回退软解**，不可在产品层要求用户手动切换硬/软解）
   - **B3**：文件夹（子项数、计算大小按钮 → 后台 `dir_size`）
4. **Info pane 标签区**：选中项所属 Tag 圆点列表（依赖 §5 `path_tags`）。
5. i18n 化 `preview_kind_title()` 等硬编码字符串。

### 步骤（概要）
- `platform-windows/src/metadata.rs`：`file_times(path) -> (created, modified, accessed)`、`file_attributes(path) -> FileAttrs`。
- `FileItem` 或 Info pane 读取时填充 metadata（refresh 时批量可选）。
- `preview.rs` 扩展 `PreviewKind::Pdf | Audio | Video | FolderSummary`。
- `info_pane.rs` 分支：多选 / 单文件 / 单文件夹 / 无选择。
- 视频最终实现约束：
  - 统一媒体后端使用 `zmwangx/rust-ffmpeg`（或其 crates 发布名 `ffmpeg-next`）。
  - Windows 渲染目标是 GPUI 窗口内的 D3D11 纹理显示，不接受 `<video>`/WebView2/普通图片控件作为最终方案。
  - 打开视频时先尝试创建硬解码器与硬件帧路径；若设备、驱动、像素格式或目标容器/编码不支持，则自动回退到软件解码，UI 只暴露统一的播放体验，不暴露“强制硬解/强制软解”开关。
  - PlaybackClock 以音频时钟为主时钟；纯视频文件则退回视频时钟。
  - 需将“硬解失败并回退软解”记录到日志，便于问题排查。

### Files 参考
- `InfoPaneViewModel.cs`、`UserControls/FilePreviews/*`

### 测试
- metadata 解析单测（Windows）；多选 size 合计单测。

---

## 11. 完整属性对话框 + 外部预览（QuickLook / Peek）

### 现状
- 右键 **Properties** 走 Shell `properties` verb（`platform-windows`），已是系统完整属性。
- **无** 应用内多页属性窗（General/Security/Hashes…）。
- **无** QuickLook / PowerToys Peek 集成。

### 目标
**阶段 A（推荐先做）— 外部预览**
- 空格键或工具栏按钮触发 **QuickLook** / **Peek** / **Seer**（按安装检测顺序尝试 `ShellExecute` 或已知 CLI）。
- 设置项：`preview_provider: none | quicklook | peek | seer`；安装检测提示。

**阶段 B — 应用内属性窗（可选，工作量大）**
- 简化版属性对话框：General（名称/大小/日期/属性 checkbox 可编辑）+ Hashes（MD5/SHA256，后台计算）。
- Security / Compatibility 仍跳转 Shell 完整属性（「更多…」按钮）。

### 步骤（概要）
- `platform-windows/src/preview_popup.rs`：`try_external_preview(paths: &[PathBuf]) -> Result<()>`。
- `PreviewFile` action（Space，文件列表 context 内）；与 Ctrl+F 搜索不冲突。
- 设置页 Preview 分组。

### Files 参考
- `Services/PreviewPopupProviders/`
- `Views/Properties/*`（阶段 B 参考）

### 边界情况
- 多选时外部预览通常只预览首项；多选显示提示。
- QuickLook 未安装 → Notification 引导下载。

---

## 12. 文件操作扩展

### 12A. Bulk rename（批量重命名）

### 现状
- 仅单文件 F2 内联重命名（`rename.rs`）。

### 目标
对话框：模式串 `{name}{ext}`、`{n}` 序号、查找替换；预览新名称列表；执行前冲突检测。

### 步骤（概要）
- `crates/files-fs/src/bulk_rename.rs`：`apply_pattern(items, pattern) -> Vec<(PathBuf, PathBuf)>`。
- `window.open_dialog` 或独立小窗；`spawn_bulk_rename` 复用 transfer 冲突流。
- Action `BulkRename`（需多选）；上下文菜单入口。

### Files 参考
- `Dialogs/BulkRenameDialog.xaml`

---

### 12B. Flatten folder（展平文件夹）

### 目标
将当前文件夹内**所有子项**（可选含子目录内文件）移动到当前目录，删除空子目录。

### 步骤（概要）
- `crates/files-fs/src/ops.rs`：`flatten_directory(dir, recursive: bool) -> Vec<(PathBuf, PathBuf)>`。
- Action `FlattenFolder`；仅当前目录为空选中或背景菜单；确认对话框。
- 可撤销：记录为 `FileOperation::Move`（§4）。

### Files 参考
- `FlattenFolderAction.cs`

---

### 12C. Browse zip as folder（虚拟压缩包浏览）

### 现状
- ZIP 仅压缩输出；打开 `.zip` 用系统默认或 CyberEditor。

### 目标
双击 `.zip` 在 CyberFiles 内以虚拟目录浏览（类似 Files `ZipStorageFolder` / `ContentPageTypes.ZipFolder`）。

### 步骤（概要）
- `BrowseLocation::Archive { path: PathBuf }`；`list_archive_entries(path) -> Vec<FileItem>`（只读）。
- 预览/打开内层文件：解压到 temp 或 streaming read（第一版：解压单文件到 `%TEMP%` 并打开）。
- 与 §1 解压共用 `archive.rs` 读 API。

### Files 参考
- `ZipStorageFolder.cs`

### 依赖
- §1 解压/读 ZIP 基础设施。

---

### 12D. 7z 压缩（扩展 §1）

### 现状
- `compress_paths_to_zip*` only；无 7z 输出。

### 目标
压缩对话框可选 ZIP / 7z（对齐 Files Create Archive）；设置默认格式。

### 步骤（概要）
- 评估 `sevenz-rust` 写 API 或调用 7z.dll（Files 捆绑 `7z.dll`）。
- `AppConfig.default_archive_format`；`spawn_compress` 分支。
- 第一版可仅 **7z 解压**（§1）+ **zip 压缩**；7z 压缩后置。

### Files 参考
- `Actions/Content/Archives/Compress/`

---

## 13. 平台与系统集成

### 13A. 设为默认文件管理器

### 目标
替换 Explorer 为默认文件夹打开方式（注册表 `Directory\Background\shell` 等 + `ProgId`），与 Files 相同需辅助进程/卸载入口。

### 步骤（概要）
- 调研 Files 的 `FilesOpenDialog` / 注册表项（`../Files` Advanced 设置）。
- 新 crate 或 `platform-windows/src/default_handler.rs`：`register_as_default_file_manager()` / `unregister()`；需管理员或用户确认 UAC。
- 设置 → Advanced →「设为默认文件管理器」按钮 + 状态检测。

### 风险
- 注册表误操作影响系统；必须提供可靠 **恢复默认（Explorer）**。

---

### 13B. 自定义打开 / 保存对话框

### 目标
其他应用「打开文件」时弹出 CyberFiles 风格对话框（Files 与 13A 共用基础设施）。

### 依赖
- 13A 同一套 shell hook / 辅助 EXE；工作量大，**排在 13A 之后**。

---

### 13C. Jump List

### 目标
任务栏图标右键：最近位置、固定文件夹、新建窗口（Windows `ICustomDestinationList`）。

### 步骤（概要）
- `platform-windows/src/jump_list.rs`：启动时 `set_jump_list(recent_paths, pinned)`。
- 与 `path_history` / `pinned_folders` 同步。

### Files 参考
- `WindowsJumpListService.cs`

---

### 13D. 开机启动（Startup with Windows）

### 目标
设置开关：当前用户 Run 注册表或 Startup 文件夹快捷方式。

### 步骤（概要）
- `platform-windows/src/startup.rs`：`set_run_at_startup(enabled: bool)`。
- `AppConfig.run_at_startup`；设置 General 页 toggle。

---

## 14. Git 集成

### 现状
- 无 Git 相关代码（`.gitignore` 仅作语法高亮扩展名）。

### 目标（分阶段，对齐 Files DevTools/Git 菜单）
**阶段 A**
- 检测目录是否为 git repo；状态栏/文件行 **badge**（Modified/Added/Deleted/Ignored）。
- 右键 / 工具栏：Open in Terminal at repo root。

**阶段 B**
- Clone dialog（URL + 目标路径）；Open repo in IDE（设置可配置 IDE 路径）。
- 常用命令：Fetch / Pull / Push（libgit2 或 `git` 子进程）。

### 步骤（概要）
- 依赖：`git2` crate 或 `std::process::Command("git")`。
- `crates/files-fs/src/git_status.rs`：`repo_status(root) -> HashMap<PathBuf, GitChange>`；文件 watcher 增量更新（debounce）。
- UI：`FileItem.git_status` 渲染；`Actions/Git/*`；可选 Git 工具栏 section。

### Files 参考
- `Actions/Git/*`、`Utils/Git/LibGit2.cs`

### 边界情况
- 非 git 目录隐藏所有 Git UI。
- 子模块 / nested repo：第一版只认根 `.git`。

---

## 15. 远程存储（FTP / MTP 等）

### 现状
- 仅本地路径 + Shell 特殊文件夹（网络邻居浏览依赖 Shell namespace，非完整 FTP 客户端）。

### 目标（长期）
- **FTP/SFTP**：`BrowseLocation::Ftp { host, path }`；列表/上传/下载（Files `FtpStorageFolder`）。
- **MTP 设备**：检测便携设备，虚拟根浏览（Files `MtpHelpers`）。

### 步骤（概要）
- 抽象 `crates/files-fs/src/storage.rs`：`trait StorageBackend { fn list(&self, path) -> ... }`；Local / Ftp / Mtp 实现。
- 侧边栏「Add network location」；连接对话框；凭据 `keyring` 存储。
- **第一版建议只做 FTP 只读浏览 + 下载**，写操作后置。

### Files 参考
- `Files.Core.Storage`、`FtpStorageFolder.cs`、`MtpHelpers.cs`

### 依赖
- 独立 epic；与 Phase A/B 并行度低，放 Phase C 末尾。

---

## 附：跨功能的公共改动清单（便于排期）

| 文件 | 涉及功能 |
|------|----------|
| `crates/files-fs/src/archive.rs` | 1, 12C, 12D |
| `crates/files-fs/src/sort.rs`(或新 `group.rs`) | 2,（5 的 Tag 排序/分组） |
| `crates/files-fs/src/history.rs`（新） | 4, 12B |
| `crates/files-fs/src/file_tag.rs`、`item.rs` | 5, 9, 10 |
| `crates/files-fs/src/recycle.rs` | 3 |
| `crates/files-fs/src/search.rs` | 9（保留 filter；新 `folder_search.rs`） |
| `crates/files-fs/src/folder_search.rs`（新） | 9 |
| `crates/files-fs/src/preview.rs` | 10 |
| `crates/files-fs/src/bulk_rename.rs`（新） | 12A |
| `crates/files-fs/src/ops.rs` | 12B |
| `crates/files-fs/src/storage.rs`（新） | 15 |
| `crates/files-fs/src/git_status.rs`（新） | 14 |
| `crates/files-app-platform-windows/src/recycle.rs`、`lib.rs` | 3 |
| `crates/files-app-platform-windows/src/search.rs`（新） | 9 |
| `crates/files-app-platform-windows/src/metadata.rs`（新） | 10 |
| `crates/files-app-platform-windows/src/preview_popup.rs`（新） | 11 |
| `crates/files-app-platform-windows/src/drag_out.rs`（新）、`lib.rs` | 8B |
| `crates/files-app-platform-windows/src/jump_list.rs`（新） | 13C |
| `crates/files-app-platform-windows/src/startup.rs`（新） | 13D |
| `crates/files-app-platform-windows/src/default_handler.rs`（新） | 13A, 13B |
| `crates/files-core/src/config.rs`（`AppConfig` + prefs + keybindings + search_history + run_at_startup） | 2,5,7,9,12D,13D |
| `crates/files-commands/src/lib.rs`（actions + action_specs + resolve_key_bindings） | 1–4,7,9,11,12,14 |
| `crates/files-commands/src/shortcuts.rs`（修一致性 / 并入注册表） | 7 |
| `crates/files-app-ui/src/file_browser.rs`（状态：group/collapsed/path_tags/display_rows/search/git_status/archive） | 2,5,9,10,12C,14 |
| `crates/files-app-ui/src/file_browser/actions.rs` + `render.rs` | 1–4,9,11,12,14 |
| `crates/files-app-ui/src/file_browser/ops.rs` | 1,3,12A,12B |
| `crates/files-app-ui/src/file_ops.rs` | 1,4,8A,12A,12B |
| `crates/files-app-ui/src/file_browser/context_menu.rs` | 1,3,5,12A,12B |
| `crates/files-app-ui/src/file_browser/render_views/{table_list,tiles}.rs` | 2,5,10,14 |
| `crates/files-app-ui/src/main_page/omnibar.rs` | 6,9 |
| `crates/files-app-ui/src/info_pane.rs` | 10 |
| `crates/files-app-ui/src/settings_view.rs` | 5,7,11,12D,13,14 |
| `crates/files-app-ui/src/app_state.rs` | 4 |
| `crates/files-app-ui/locales/app.yml` | 全部 |

### 验证流程（每个功能完成后）
```powershell
cargo build
cargo test -p files-fs        # 纯逻辑单测（archive/group/history/omnibar）
```
UI 行为以 `scripts/debug/cyberfiles.ps1`（或 `scripts/build-debug-cyberfiles.ps1`）跑起来手动验收。

---

## 19. 双栏（Dual Pane，对照 Files `ShellPanesPage`）

### 现状（CyberDesktop）

- `ShellPanes`：主/副 `PaneShell` 常驻；`dual_pane` 控制显示；副栏点击聚焦；`open_path_in_secondary_pane` + 右键「在新窗格打开」（仅文件夹）。
- 布局：可拖 `PaneSplitDrag` + `split_ratio` / `arrangement`（禁止第三层 `h_resizable` 嵌套）。
- 会话：`SessionPaneLayout { primary_tab, secondary_tab, arrangement, split_ratio, dual_pane, active_side }`；`session_tabs` 存主栏路径。
- 命令：`files-commands`（`ToggleDualPane`、`SplitPane*`、`ArrangePanes*`、`FocusOtherPane`、`CloseActivePane`、`OpenInNewPane`）；查看菜单 / 标签栏右键 / 首页与设置页空白右键 / 侧栏与首页项「在新窗格打开」。

### 目标（对齐 Files 行为）

| 能力 | Files | CyberDesktop 目标 |
|------|-------|-------------------|
| 切换双栏 | Ctrl+Shift+S | D1 |
| 垂直/水平分栏 | Alt+Shift+V / H | D2 + D4 |
| 聚焦另一栏 | Ctrl+Shift+Right | D1 |
| 关闭当前栏 | Ctrl+Alt+W | D1 |
| 文件夹→副栏 | Ctrl+Shift+Enter | D1 |
| 可拖比例 | GridSplitter | D2（自定义 splitter） |
| 窄窗收起 | 宽度 ≤750px | D3 |
| 会话 | 左右路径 + 排列 | D0 起 `primary_tab` + `secondary_tab` |

双栏**不**镜像导航；Omnibar / 侧栏 / 状态栏 / Info pane 仅跟 **active pane**（与 Files `ActivePaneOrColumn` 一致）。

### D0 — 基础稳固（实施要点）

1. **`SessionPaneLayout` 扩展**（`config.rs`）：`primary_tab`、`arrangement`（`vertical`/`horizontal`）、`split_ratio`（默认 `0.5`，D2 使用）。
2. **`persist_session`**：`session_tabs[i]` = 主栏编码路径；`capture_shell_layout` 同时写入 `primary_tab` / `secondary_tab`。
3. **`restore_layout`**：恢复主栏 + 副栏路径与 `active_side`。
4. **焦点**：`activate_pane` 时清空非活跃栏 `FileBrowser` 选择。
5. **`toggle_dual_pane`**：开启时副栏导航到主栏当前位置；关闭时仅隐藏副栏。
6. **工具栏**：双栏开启时按钮高亮（accent）。

### D1 — 命令与入口

- `files-commands`：`ToggleDualPane`、`FocusOtherPane`、`CloseActivePane`；`OpenInNewPane` 迁入并注册默认键（可配置）。
- `MainPage` `.on_action` 绑定；`settings.actions.*` i18n。
- 列表/详细信息视图：**Ctrl+Shift+Enter** → `open_path_in_secondary_pane`（目录项）。

### D2 — 可拖分栏 + 排列方向

- `shell_panes.rs`：中间拖拽手柄；比例写 `split_ratio`；竖向 `v_flex` 布局。
- 双击手柄均分；副栏宽度过窄自动 `close_other_pane`。
- **禁止**在 `shell_panes` 内使用第三层 `gpui_component::h_resizable`。

### D3 — 窄窗

- 常量 `MULTI_PANE_WIDTH_THRESHOLD = 750`；窄窗隐藏副栏并缓存 `secondary_tab`；拉宽恢复。

### D4 — 设置与菜单

- General：`always_open_dual_pane_in_new_tab`、`shell_pane_arrangement`、`show_open_in_new_pane`（`config.rs`）。
- 应用菜单 / 标签栏右键：垂直/水平分栏子菜单。

### 测试要点

- 双栏开关 → 重启 → 左右路径、活跃侧正确。
- 副栏聚焦时保存会话 → 重启后主栏路径不丢。
- 切换双栏无 stack overflow；快捷键在设置页可改键。
- D2 后：拖 splitter、双击均分、拖窄关副栏。
