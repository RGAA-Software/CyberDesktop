# CyberEditor 引擎重写 —— 总进度文档

目标：把 `cybereditor` 的编辑面从 `gpui-component` 的 `InputState` 换成自建的高性能编辑引擎，
对标 Notepad++/Scintilla：秒开大文件、滚动不卡、任意编码、全局搜索。

架构边界：

```text
crates/text-engine/   纯逻辑、无 GPUI：buffer / encoding / loader / selection / history / document / search / syntax
crates/ui-editor/     GPUI 自绘编辑 Element + 输入 + 标签/状态栏/查找面板（外壳继续用 gpui-component）
```

核心原则：
- Buffer 用 rope（`ropey`），所有尺寸/坐标查询 O(log n)。
- 渲染只处理可见行（视口虚拟化）。
- tree-sitter 增量解析 + 仅可见视口着色，不依赖 LSP。
- 加载用 mmap + 流式解码，无巨型中间 String。

---

## 进度

### 已完成

- [x] `crates/text-engine` 脚手架 + 接入 workspace
- [x] `buffer.rs` —— rope `TextBuffer`：编辑、行/字符/字节互转、`EditSummary`、revision（含测试）
- [x] `encoding.rs` —— BOM + `chardetng` 探测，`encoding_rs` 解码/编码，行尾探测（含测试）
- [x] `loader.rs` —— mmap + 流式解码进 `RopeBuilder`（含 UTF-8/GBK/BOM/CRLF/空文件测试）

### 已完成（续）

- [x] `selection.rs` —— 光标/选区模型（anchor+head 字符偏移，多选区合并）
- [x] `history.rs` —— 撤销/重做事务栈（多编辑、逆操作、redo 失效）
- [x] `document.rs` —— `Document`：buffer + selections + history + encoding + line_ending + language + dirty + path；统一编辑入口；输入合并撤销；保存按编码+行尾写盘
- [x] `search.rs` —— 缓冲区内查找/替换（字面量 + 正则、大小写、整词、捕获组替换、行级流式 find_next/prev）
- [x] `global_search.rs` —— ripgrep 库（`grep-searcher`/`grep-regex`/`ignore`）跨文件夹搜索（gitignore 感知、二进制跳过、列号）
- [x] `syntax.rs` —— tree-sitter 高亮（rust/json/python/javascript/c/bash），按可见字节区间查询、重叠消解
- [x] `ui-editor/editor_view.rs` —— 自建 GPUI `Element`（`EditorCanvas`）：记录 bounds、只 shape/paint 可见行（视口虚拟化）、按 tree-sitter span 上色、绘制行号槽/选区/光标
- [x] 像素级命中测试：`ShapedLine::closest_index_for_x` 把鼠标坐标映射到字符偏移；点击置光标、Shift+点击/拖拽选区
- [x] 滚轮垂直滚动 + 编辑/移动后自动滚动到光标可见
- [x] 可视垂直滚动条：滑块大小/位置随内容比例计算，支持拖动滑块、点击轨道翻页（内容不溢出时自动隐藏）
- [x] 文本/IME 输入走 `EntityInputHandler`（`replace_text_in_range`/marked range/`bounds_for_range`/`character_index_for_point`），rope 级 UTF-16 互转
- [x] 键盘编辑接到 `Document`（回车、退格/删除、方向键、Home/End、Ctrl+A/Z/Y/S；可打印字符走 IME 输入路径）
- [x] 启动自动聚焦、点击聚焦，键盘即时可用
- [x] 引擎视图自带头部状态栏（文件名/语言/编码/行尾/Ln,Col）+ 文件加载/保存
- [x] `cybereditor` 二进制已切到 `EngineEditor`，整体编译通过并运行验证
- [x] gpui-component 标题栏（可拖动窗口 + 最小化/最大化/关闭）：`EditorShell` 始终编译，新增 `open_editor_window` 强制走编辑器外壳，规避 `full-app` 特性合并
- [x] 标题栏菜单（File / Edit / Selection / View / Help）：复用 `AppMenuBar`，动作派发到 `EngineEditor`（新建/打开/保存/退出、撤销/重做、剪切/复制/粘贴、查找/替换、缩进/反缩进/注释切换、行号开关、关于面板）；剪贴板、缩放（`Ctrl+=/-/0`）、整行选择（`Ctrl+L`）一并接入
- [x] 打开/新建/保存/另存为（`Ctrl+O/N/S/Shift+S`，复用 `rfd` 原生对话框；另存为按新扩展名切换语言高亮）
- [x] 查找/替换栏（`Ctrl+F`/`Ctrl+H`、`F3`/`Shift+F3`、`Esc`）：接 `search.rs`，大小写/整词/正则开关、上一处/下一处、单个替换、全部替换、命中计数（x of n）
- [x] **tree-sitter 增量 reparse（高性能核心）**：`Document` 累积字节级 `EditSummary`（含 row/col 点），编辑后 `Tree::edit` + 复用旧树增量重解析；解析与高亮均通过 rope 分块回调读取（`parse_with_options` + `RopeProvider` 的 `TextProvider`），**每次按键不再 `to_string()` 整篇**，大文件编辑零整文拷贝
- [x] Go to Line（`Ctrl+G`，菜单 View → Go to Line…）：浮层输入行号、回车跳转并滚动到位
- [x] 横向滚动 + 可视水平滚动条：滚轮（`Shift+滚轮` 横向）、内容区裁剪（`with_content_mask` 防止盖住行号槽/竖向滚动条）、光标横向自动可见（仅 shape 光标所在单行计算 x）、水平滚动条滑块拖动/轨道翻页（按可见行最大宽度估算滚动范围）
- [x] 全局搜索结果面板 Find in Files（`Ctrl+Shift+F`，菜单 Edit → Find in Files…）：接 `global_search.rs`，后台线程执行（`background_executor`，generation 防过期结果回填），按文件分组、点击结果跳转打开并定位行，大小写/整词/正则开关
- [x] 多光标 UI（引擎已支持多选区）：`Ctrl+D` 选中光标处单词 / 追加下一处匹配（环绕查找）、`Alt+点击` 增加光标、`Esc` 折叠回单光标；`prepaint` 渲染全部选区带与光标
- [x] **多光标键盘移动 + 多点编辑**：方向键/Home/End/上下行对**每个**光标逐一映射并归一化合并；无 IME marked range 时多光标输入直接 `document.insert` 在所有光标处同步插入，退格/删除天然作用于全部光标
- [x] 查找/替换栏字段内编辑：左右/Home/End 移动字段内光标、中间插入、退格/前向删除（`Delete`），渲染按光标位置插入竖条
- [x] **多标签（Tab 栏）**：每个文档一个标签（`park/activate` 交换活动标签的 live 状态，非活动标签停放在 `TabSlot`，切换零拷贝 move）；`Ctrl+T`/`Ctrl+N` 新建、`Ctrl+W` 关闭、`Ctrl+Tab`/`Ctrl+Shift+Tab` 循环、点击切换、`×` 关闭、`+` 新建；标签标题含脏标记 `•`；打开文件优先复用已开标签/空白标签
- [x] 最近文件（MRU）：打开/保存自动入栈（去重、上限 12），`Ctrl+E` 下拉浮层，点击重新打开
- [x] 外部修改检测：后台 `background_executor` 计时器每 ~1.5s 重新 `stat` 活动文件（mtime+len，廉价、无每文件 watcher 线程），变更后顶部横幅提示 Reload / Ignore；自身保存后刷新指纹避免误报
- [x] **软换行 Word Wrap（高性能）**：`View → Word Wrap`/菜单切换；用 `shape_text(wrap_width)` 的 `WrappedLine` 渲染（自身绘制全部子行），选区按子行铺带、光标/IME 用 `position_for_index` 定位、命中用 `closest_index_for_position`；视口按**文档行 + 子行像素偏移**锚定（`wrap_top_line`/`wrap_top_off`），滚动/翻页只测量视口邻近行 —— **O(视口)**，巨型文件不做整篇换行扫描；竖向滚动条按文档行比例近似、横向滚动条软换行下自动隐藏
- [x] CJK 宽字符命中：命中测试与光标/选区 x 坐标均走真实字形 `closest_index_for_x` / `x_for_index`（无等宽假设），混排已正确

引擎层共 8 个模块、**38 个单测全过**；编辑面功能完整（键鼠、滚动、选区、IME、查找替换、文件操作、全局搜索、多光标键鼠、多标签、最近文件、外部修改检测、软换行）。

### 后续可选增强

- [ ] 软换行视口竖向滚动条精确到子行（当前按文档行近似，已可平滑滚动）
- [ ] 关闭含未保存改动的标签时弹确认
- [ ] **横向视口虚拟化**（超长单行 / minified bundle）—— 详见 [cybereditor-horizontal-viewport-todo.md](./cybereditor-horizontal-viewport-todo.md)

---

## 验证方式

每个引擎模块都带单测，按步编译验证：

```powershell
cargo test -p cyber-desktop-text-engine
cargo build -p cyber-desktop --bin cyber_editor
cargo run  -p cyber-desktop --bin cyber_editor -- <file>
```

---

## 模块设计要点

### selection.rs
- `Cursor { anchor: usize, head: usize }`（字符偏移）；无选区时 anchor==head。
- `range()` 归一化为 `start..end`；`is_empty()`；`min()/max()`。
- 多光标用 `Vec<Cursor>`，编辑后合并重叠、按位置排序。

### history.rs
- 一次编辑 = `Transaction { edits: Vec<EditOp>, before_selections, after_selections }`。
- `EditOp` 记录被替换区间与新旧文本，支持正向 apply 与逆向 invert。
- 连续输入合并到同一事务（按时间/类型），撤销以事务为单位。

### document.rs
- 唯一的编辑入口：`insert`/`delete`/`replace_selections`，内部更新 buffer、selections、history、dirty、并产出 `EditSummary` 列表供 syntax 增量更新。
- 保存：按 `EncodingInfo` + `LineEnding` 编码写盘。

### syntax.rs
- 持有 `tree_sitter::Parser` + 当前 `Tree`；编辑时 `tree.edit(InputEdit)` 后增量 reparse。
- 提供 `highlights(visible_byte_range) -> Vec<(Range, HighlightId)>`，只查可见区间。

### 渲染 Element
- 输入：滚动偏移、视口尺寸、行高 → 计算 `first_line..last_line`。
- 仅对可见行取 rope 行文本 + 高亮 span，shape 并 paint；行号槽单独绘制。
- 命中测试：像素坐标 ↔ (line, column) ↔ 字符偏移。
