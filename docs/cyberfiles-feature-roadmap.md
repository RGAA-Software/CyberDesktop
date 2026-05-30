# CyberFiles 复刻 Files 功能落地实现文档

本文针对以下功能，给出**逐步可执行**的实现方案。每一节都标注了需要改动的真实文件、函数签名、数据结构、配置项、i18n key、边界情况与测试要点，目标是照着做即可落地。

> 说明：用户列出的「4. Undo/Redo 文件操作历史」与「6. 撤销/重做文件操作」是同一功能，已合并为第 4 节。本文共 **8 个功能**。

实现顺序建议（依赖关系 + 性价比）：

1. 归档解压（独立、无依赖）
2. 分组 Group by（独立）
3. 回收站 还原/清空（依赖 platform-windows COM）
4. Undo/Redo 文件操作历史（需要先统一文件操作入口）
5. 文件标签系统（在现有虚拟标签上扩展）
6. 路径自动补全（后端已就绪，只差 UI）
7. 快捷键自定义（需要配置化 KeyBinding）
8. OS↔资源管理器 拖拽（最难，涉及 OLE 拖放）

通用约定：
- 所有用户可见文案走 `rust_i18n::t!`，并在 `crates/ui/locales/app.yml`（含 `en`/`zh-CN`/`zh-Hant`）补 key。
- 纯逻辑放 `crates/fs`（可单测，不依赖 GPUI）；Windows 专有能力放 `crates/platform-windows`；UI 编排放 `crates/ui`。
- 动作（action）统一在 `crates/commands/src/lib.rs` 的 `actions!` 注册，键位在 `file_browser_key_bindings()`，在 `crates/ui/src/file_browser/render.rs` 的 `.on_action(...)` 链中挂处理函数（处理函数写在 `crates/ui/src/file_browser/actions.rs`）。
- 后台耗时操作仿照 `crates/ui/src/file_ops.rs` 的 `spawn_*` 模式：`cx.spawn` + `background_spawn` + `TransferStatusGlobal` 进度 + `Notification`。

---

## 1. 归档解压（ZIP / 7z / tar）

### 现状
- `crates/fs/src/archive.rs` 仅实现 ZIP **压缩**（`compress_paths_to_zip*`），无解压。
- UI 入口 `CompressItems` action（`crates/commands/src/lib.rs`），`compress_items()`（`crates/ui/src/file_browser/ops.rs`）+ `spawn_compress()`（`crates/ui/src/file_ops.rs`）。
- 上下文菜单可见性开关：`context_menu_show_compress`（`crates/core/src/config.rs`）。

### 目标
对选中的 `.zip` / `.7z` / `.tar` / `.tar.gz` / `.tgz` 提供三种解压（对齐 Files）：
- **Extract here**：解压到当前目录。
- **Extract to <名字>\\**：解压到与压缩包同名子文件夹。
- **Extract...**（可后置）：弹目标选择对话框。

### 依赖（Cargo）
在 `crates/fs/Cargo.toml` 增加：
```toml
sevenz-rust = "0.6"     # 7z 解压（纯 Rust）
tar = "0.4"             # tar
flate2 = "1"            # gzip (.tar.gz/.tgz)
```
`zip` crate 已在依赖中（用于读取 ZIP）。

### 步骤

**(1) 在 `crates/fs/src/archive.rs` 增加解压 API。**

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

**(3) 新增 action。** 在 `crates/commands/src/lib.rs` 的 `actions!` 里加 `ExtractHere`、`ExtractToFolder`（`Extract` 对话框版可后置）。无需默认键位。

**(4) UI 编排。** 在 `crates/ui/src/file_browser/ops.rs` 新增：
```rust
pub(super) fn extract_selection(&mut self, to_subfolder: bool, window: &mut Window, cx: &mut Context<Self>);
```
- 取 `selected_paths_vec()` 中 `is_archive_path` 为真的项（多选支持）。
- 目标目录：`to_subfolder` 时 `extract_to_child_dir`，否则 `operation_directory()`。
- 调新写的 `spawn_extract(...)`。

在 `crates/ui/src/file_ops.rs` 新增 `spawn_extract`，**完全照搬 `spawn_compress` 结构**：开 `TransferStatusGlobal::begin`、后台线程跑 `extract_archive_cancellable`、`mpsc` 回传进度、结束 `end/cancel/fail` + `Notification`，最后若 `browser.shows_directory(dest)` 则 `reload()`。

**(5) 处理函数与挂载。** `crates/ui/src/file_browser/actions.rs` 加 `on_extract_here` / `on_extract_to_folder`；`render.rs` 的 `.on_action` 链补两行。

**(6) 上下文菜单项。** 在 `crates/ui/src/file_browser/context_menu.rs` 选中项菜单里，当 `selected` 全部/部分为压缩包时插入「Extract here」「Extract to <name>」（参考现有 Compress 项的插入方式与 `context_menu_show_compress` 开关；可新增 `context_menu_show_extract`，默认 true）。

**(7) i18n。** `files.extract.here` / `files.extract.to_folder` / `files.extract.extracting`（count）/ `files.extract.done` / `files.extract.failed` / `files.transfer.cancelled`（已存在）。

### 边界情况
- 目录穿越攻击（zip slip）：必须用 `enclosed_name()`/校验解析后路径仍在 `dest_dir` 内。
- 加密压缩包：`zip`/`sevenz-rust` 会报错，捕获后提示「需要密码」（密码对话框可后置）。
- 同名冲突：第一版「Extract here」直接覆盖或跳过既有文件即可；进阶可复用 `ConflictResolution` 流程。
- 空压缩包 / 损坏文件：返回 `Err`，UI 提示失败。

### 测试
- `crates/fs/src/archive.rs` 加 `#[cfg(test)]`：构造临时 zip（用现有压缩 API 造）→ 解压 → 校验文件树一致；测试 `extract_to_child_dir` 对 `a.zip`/`a.tar.gz` 的命名；测试 zip slip 被拒绝。

---

## 2. 分组（Group by）

### 现状
- 只有排序：`crates/fs/src/sort.rs`（`SortOption`/`SortDirection`/`SortPreferences`/`sort_items`）。
- 列表渲染：`display_items: Vec<FileItem>` 是平铺列表，经 `apply_filter()` 生成（`crates/ui/src/file_browser/navigation.rs`）。
- 视图渲染：`crates/ui/src/file_browser/render_views/table_list.rs`、`tiles.rs`、`columns.rs`，用 `v_virtual_list` + `item_sizes`。

### 目标
对齐 Files 的 Group by：None / Name(首字母) / DateModified / DateCreated / Size(分桶) / Type / Tag。分组后在列表中插入**分组标题行**，标题可折叠，组内仍按当前排序。

### 设计要点
分组本质是：在 `display_items` 之上再生成「带分组头的渲染序列」。推荐引入一个渲染行枚举，避免大改虚拟列表：

**(1) `crates/fs/src/sort.rs`（或新文件 `group.rs`）增加：**
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

**(2) 生成分组渲染序列。** 在 `crates/fs/src/sort.rs` 增加：
```rust
pub struct FileGroup { pub key: String, pub title: String, pub item_indices: Vec<usize> }

/// 在已排序的 items 上分组；组的先后顺序遵循 direction（日期/大小按桶序，名称/类型按字典序）。
pub fn group_items(items: &[FileItem], group: GroupOption, dir: SortDirection) -> Vec<FileGroup>;
```

**(3) FileBrowser 状态。** `crates/ui/src/file_browser.rs` 的 `FileBrowser` 增加：
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
- `crates/commands/src/lib.rs` 加 `GroupByNone/Name/Modified/Created/Size/Type/Tag` actions。
- `actions.rs` 加 `on_group_by_*` → 设 `group_option`、`apply_filter()`、`persist_prefs()`、`cx.notify()`。
- `render.rs` 挂 `.on_action`。
- 持久化：`crates/core/src/config.rs` 的 `AppConfig` 加 `#[serde(default)] pub file_group_option: Option<String>`，扩展 `save_file_browser_prefs(...)` 签名（当前是 5 参，加第 6 参 group），并加常量 `GROUP_NONE/NAME/...` 与 `file_group_from_config()`。`FileBrowser::with_options` 启动时读取。
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
- 浏览：`BrowseLocation::RecycleBin`、`read_recycle_bin()`（`crates/fs/src/recycle.rs`）→ `list_recycle_bin_entries()`（`crates/platform-windows/src/recycle.rs`，走 Shell namespace）。
- `RecycleBinEntry { display_name, shell_path, size, modified }`，`shell_path` 是解析路径，可用于 Shell verbs。
- 删除到回收站：`recycle_paths()`（`crates/fs/src/ops.rs`，用 `trash` crate）。
- **缺**：还原单项 / 还原全部 / 清空回收站。

### 目标
- **Restore**：对回收站中选中项执行 Shell `restore` 动词，还原到原位置。
- **Restore all**：对所有项执行 restore。
- **Empty Recycle Bin**：清空（带确认）。

### 步骤

**(1) platform-windows 增加 COM 能力。** 新增 `crates/platform-windows/src/recycle.rs` 中函数（或新文件 `recycle_ops.rs`）：
```rust
/// 清空回收站（带 Win32 进度 UI）。
pub fn empty_recycle_bin() -> anyhow::Result<()>;   // SHEmptyRecycleBinW(None, None, 0)

/// 还原指定回收站项（按 shell_path 解析 PIDL 后执行 "restore"/"undelete" 动词）。
pub fn restore_recycle_bin_items(shell_paths: &[PathBuf]) -> anyhow::Result<()>;
```
实现：
- `empty_recycle_bin`：直接调用 `windows::Win32::UI::Shell::SHEmptyRecycleBinW`（flags 可用 `SHERB_NOCONFIRMATION` 因为我们自己弹确认，或不带让系统确认）。
- `restore_recycle_bin_items`：枚举回收站 `IShellFolder`（复用 `list_recycle_bin_entries_inner` 的绑定逻辑），对匹配 `shell_path` 的 PIDL 用 `IContextMenu`（`GetUIObjectOf` 取 `IID_IContextMenu`）查找并 invoke `restore`（旧称 `undelete`）动词。这是与 `crates/platform-windows/src/context_menu.rs`/`shell.rs` 同一套 `IContextMenu::InvokeCommand` 模式，可复用其 verb 调用封装。
- 在 `crates/platform-windows/src/lib.rs` `pub use` 导出这两个函数，并在 `mod stubs` 里加非 Windows 空实现。

**(2) fs 层薄封装**（可选，便于跨平台/测试）：`crates/fs/src/recycle.rs` 加：
```rust
#[cfg(windows)] pub fn empty_recycle_bin() -> anyhow::Result<()> { cyber_desktop_platform_windows::empty_recycle_bin() }
#[cfg(windows)] pub fn restore_recycle_items(paths: &[PathBuf]) -> anyhow::Result<()> { ... }
// 非 windows 返回 Ok(())/bail
```

**(3) Actions。** `crates/commands/src/lib.rs` 加 `RestoreRecycleItems`、`RestoreAllRecycleItems`、`EmptyRecycleBin`。

**(4) 处理函数。** `crates/ui/src/file_browser/actions.rs`：
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
- 文件操作分散：`copy_items/cut_items/paste_items`（`ops.rs`）、`spawn_file_transfer`/`spawn_paste_from_clipboard`（`file_ops.rs`）、`rename_path`（`crates/fs/src/ops.rs` + `rename.rs`）、`create_directory/create_file`、`recycle_paths/delete_paths`、`create_folder_from_selection`。
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

**(1) fs 层定义历史模型。** 新建 `crates/fs/src/history.rs`：
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

**(2) 全局历史存放。** 历史是「每窗口」还是「全局」？Files 是每窗口。最简：放进程级 `App` global（仿 `AppFileClipboard`，见 `crates/ui/src/app_state.rs`）。新增 `AppOperationHistory`，提供 `record/undo/redo/can_*` 静态方法（内部 `cx.global_mut`）。

**(3) 统一记录点。** 关键改造——所有写操作完成后调用 `AppOperationHistory::record`：
- `spawn_file_transfer` / `spawn_paste_from_clipboard`（`file_ops.rs`）：在 `run_transfer_with_conflicts` 成功路径收集实际发生的 `(src,dst)`（move）或 `dst`（copy），完成后 record。注意只记录真正 transferred 的项（跳过 skip 的）。
- `rename`（`rename.rs` 的 commit 处）：record `Rename`。
- `create_new_folder/create_new_file`（`ops.rs`）：record `Create`。
- 删除（`confirm_delete_inner` 的 recycle 分支）：record `Recycle`；**永久删除分支不 record**。
- `create_folder_from_selection`：本质是 Create + Move，可记录为复合（简化：记录 Move，新文件夹的删除留给手动）。第一版可只记录 Move 部分。

**(4) Actions + 键位。**
- `crates/commands/src/lib.rs` 加 `UndoOperation`、`RedoOperation`；键位 `ctrl-z` / `ctrl-y`（mac `cmd-z`/`cmd-shift-z`）。注意：当前没有全局文本输入抢占 Ctrl+Z 的问题需确认（重命名 InputState 激活时应让输入框优先——在 `on_undo` 里判断 `self.renaming.is_some()` 则 `cx.propagate()`）。
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
- 配置：`FileTagConfig { name, color: Option<String>, paths: Vec<String> }`（`crates/core/src/config.rs`），`AppConfig.file_tags`。注释明确「full tag system is future work」。
- 浏览：`BrowseLocation::FileTag { tag_name }` → `paths_for_file_tag` → `file_items_for_tag_paths`（`crates/fs/src/file_tag.rs`）。
- 赋值/移除：`assign_paths_to_file_tag` / `remove_paths_from_file_tag`（`crates/ui/src/shell/preferences.rs`），入口在 `context_menu.rs`（受 `context_menu_show_file_tags` 控制）。
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

**(1) 建立「路径→标签」反查。** 当前数据是「标签→路径列表」，渲染文件行时需反查。新增 `crates/fs/src/file_tag.rs`：
```rust
pub struct TagRef { pub name: String, pub color: Option<String> }
/// 给定一批 (tag_name, color, paths)，构造 path→Vec<TagRef> 映射。
pub fn build_path_tag_index(tags: &[(String, Option<String>, Vec<PathBuf>)]) -> HashMap<PathBuf, Vec<TagRef>>;
```
`FileBrowser` 持有 `path_tags: HashMap<PathBuf, Vec<TagRef>>`，在 `refresh()`/标签变更时从 `load_config().file_tags` 重建。

**(2) 渲染标签标记。** 在 `table_list.rs`（详情视图加一列或名字后缀）、`tiles.rs`（卡片角标）渲染 `path_tags.get(&item.path)`，用 `color` 画圆点。颜色字符串解析复用 `crates/ui/src/color_icon.rs` 的配色或主题色映射；`FileTagConfig.color` 建议存 hex（如 `#E53935`）。

**(3) 颜色选择 UI。** `settings_view.rs` 标签编辑行加一组预设色板按钮（Files 用固定调色板），点击写 `FileTagConfig.color` 并 `save_config`。预设色定义为常量数组。

**(4) 多标签 toggle 菜单。** `context_menu.rs` 的「Tags」子菜单：对每个已定义标签显示「✓/空 + 颜色点 + 名称」，点击 toggle（在该标签 paths 中增删当前选择）。复用 `assign_paths_to_file_tag`/`remove_paths_from_file_tag`，并据当前选择是否已在该标签内决定 toggle 方向。变更后重建 `path_tags`、`refresh`。

**(5) 排序/分组接入。** 
- `SortOption::Tag`：比较时用「该 item 的首个标签名」（无标签排最后）。需要 `sort_items` 能访问 `path_tags`——可改为在 UI 层分组前注入，或给 `FileItem` 增加 `tags: Vec<TagRef>` 字段（更直接）。**推荐**：在 `refresh()` 生成 `items` 后，遍历填充 `FileItem.tags`（需在 `crates/fs/src/item.rs` 给 `FileItem` 加 `#[serde skip]`-style 字段 `pub tags: Vec<String>`，默认空）。这样 sort/group 都能用。
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
- 后端**已就绪**：`omnibar_path_suggestions(query, path_history)`（`crates/fs/src/omnibar.rs`）返回 `Vec<OmnibarPathSuggestion>`：空输入给历史，否则给子目录匹配。
- 历史已持久化：`AppConfig.path_history`，`crates/core/src/path_history.rs`。
- Omnibar 路径编辑：`enter_omnibar_path_edit` / `submit_omnibar_path`（`crates/ui/src/main_page/omnibar.rs`）。
- 面包屑 chevron 下拉已有完整实现（`breadcrumb_flyout.rs`），可作为 UI 参照。
- **缺**：路径编辑态下的补全下拉 UI 未接线。

### 目标
在 Omnibar 进入路径编辑模式后，随输入实时弹出补全列表（历史 + 子目录），支持上下键选择、Enter 跳转、Esc 关闭、Tab 补全到高亮项。

### 步骤

**(1) 定位 Omnibar 输入状态。** 在 `crates/ui/src/main_page/omnibar.rs`（及承载 omnibar 输入 `InputState` 的结构，可能在 `crates/ui/src/main_page/mod.rs` 或 `omnibar.rs`）找到路径编辑用的 `InputState` 与其 `InputEvent` 订阅。

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
- 键位**硬编码**：`file_browser_key_bindings()`（`crates/commands/src/lib.rs`）在 `init()` 里 `cx.bind_keys(...)`。
- 设置页「Actions」是**只读**参考表：`shortcut_reference()`（`crates/commands/src/shortcuts.rs`）→ `settings_view.rs` 展示。
- 文档与实际有不一致（`shortcuts.rs` 写 Ctrl+2=grid/Ctrl+3=columns，实际 `lib.rs` 是 Ctrl+2=List / Ctrl+3=Grid / Ctrl+4=Columns）——**应先修这个 bug**。

### 目标
对齐 Files `ActionsSettingsService`：每个 action 可在设置页改键位，持久化，冲突检测，可重置默认。

### 步骤

**(0) 先修文档一致性 bug。** 校正 `shortcut_reference()` 使其与 `file_browser_key_bindings()` 完全一致（含 List/Grid/Cards/Columns、ReopenTab、ToggleShowFileExtensions 等缺失项）。这是把「真相源」统一的前置工作。

**(1) 建立 action 注册表（单一真相源）。** 在 `crates/commands/src/lib.rs` 定义稳定的 action 标识与默认键位的统一表：
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
- 仅应用内拖拽：payload `DraggedFilePaths`（`crates/ui/src/drag.rs`，注释明确「not OS shell drag type」）。
- 应用内 drop 逻辑成熟：`handle_drop` / `handle_drop_on_item`（`crates/ui/src/file_browser/core.rs`）→ `spawn_file_transfer`。
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
// crates/platform-windows/src/drag_out.rs
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

## 附：跨功能的公共改动清单（便于排期）

| 文件 | 涉及功能 |
|------|----------|
| `crates/fs/src/archive.rs` | 1 |
| `crates/fs/src/sort.rs`(或新 `group.rs`) | 2,（5 的 Tag 排序/分组） |
| `crates/fs/src/history.rs`（新） | 4 |
| `crates/fs/src/file_tag.rs`、`item.rs` | 5 |
| `crates/fs/src/recycle.rs` | 3 |
| `crates/platform-windows/src/recycle.rs`、`lib.rs` | 3 |
| `crates/platform-windows/src/drag_out.rs`（新）、`lib.rs` | 8B |
| `crates/core/src/config.rs`（`AppConfig` + `save_file_browser_prefs` + keybindings + file_group_option） | 2,5,7 |
| `crates/commands/src/lib.rs`（actions + action_specs + resolve_key_bindings） | 1,2,3,4,7 |
| `crates/commands/src/shortcuts.rs`（修一致性 / 并入注册表） | 7 |
| `crates/ui/src/file_browser.rs`（FileBrowser 状态：group/collapsed/path_tags/display_rows） | 2,5 |
| `crates/ui/src/file_browser/actions.rs` + `render.rs`（挂 on_action） | 1,2,3,4 |
| `crates/ui/src/file_browser/ops.rs`（extract/restore/empty 编排） | 1,3 |
| `crates/ui/src/file_ops.rs`（spawn_extract、记录历史、外部 drop） | 1,4,8A |
| `crates/ui/src/file_browser/context_menu.rs`（extract / 回收站 / 多标签 toggle） | 1,3,5 |
| `crates/ui/src/file_browser/render_views/{table_list,tiles}.rs`（分组头、标签点） | 2,5 |
| `crates/ui/src/main_page/omnibar.rs`（补全下拉） | 6 |
| `crates/ui/src/settings_view.rs`（标签颜色、快捷键编辑） | 5,7 |
| `crates/ui/src/app_state.rs`（AppOperationHistory global） | 4 |
| `crates/ui/locales/app.yml`（en/zh-CN/zh-Hant 新 key） | 全部 |

### 验证流程（每个功能完成后）
```powershell
cargo build
cargo test -p cyber-desktop-fs        # 纯逻辑单测（archive/group/history/omnibar）
```
UI 行为以 `scripts/debug/cyberfiles.ps1`（或 `scripts/build-debug-cyberfiles.ps1`）跑起来手动验收。
