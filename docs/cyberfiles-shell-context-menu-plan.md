# CyberFiles Shell 右键菜单 — 生产方案与 Warm-up 计划

本文档描述 CyberFiles 在 Windows 上实现 **真实 Shell 右键菜单** 的完整上线方案，包括根因、架构决策、Hybrid 分层、Warm-up、Gate 测试、API/UI 变更与排期。

**状态：** ✅ 已生产集成。`query_shell_context_menu_items` 与 `warm_up_query_context_menu` 已改为 Hybrid 路径；`ThreadWithMessageQueue` 改用原生 Win32 线程，避免被遗弃的 STA 线程锁住测试/应用进程；H1–H3 改为子进程隔离执行；Gate 0–5 + H1–H3 单独运行时均通过本机验证。

**相关代码（当前）：**

| 区域 | 路径 |
|------|------|
| Layer A/B 合并 + 公共 API | `crates/app-platform-windows/src/context_menu.rs` |
| Hybrid session（Layer A + Layer B 路由） | `crates/app-platform-windows/src/hybrid_shell_session.rs` |
| Shell session（Files 模型，现复用） | `crates/app-platform-windows/src/shell_menu_session.rs` |
| COM / STA 线程 | `crates/app-platform-windows/src/com.rs` |
| 逐 handler 探测（Layer B） | `crates/app-platform-windows/src/per_handler_shell.rs` |
| Hybrid 查询与 Warm-up（测试/诊断模块） | `crates/app-platform-windows/src/hybrid_shell_menu.rs` |
| UI 右键 / 缓存 / invoke | `crates/files-ui/src/file_browser/context_menu_state.rs` |
| UI flyout 构建 | `crates/files-ui/src/file_browser/context_menu.rs` |
| 启动 Warm-up 调用 | `crates/files-ui/src/lib.rs` |
| Hang 复现 / test2 | `crates/app-platform-windows/src/context_menu.rs` → `hang_repro_tests` |

**参考实现（外部）：**

- [files-community/Files](https://github.com/files-community/Files) — `ContextMenu.cs`、`ThreadWithMessageQueue.cs`
- [Filedini ShellContextMenu.cs](https://github.com/YoshihiroIto/Filedini-public/blob/main/Source/_70_ServiceImplements/Windows/ShellContextMenu.cs)
- [cignoir/win-context-menu](https://github.com/cignoir/win-context-menu) — Rust API 模块划分
- Raymond Chen — [Hosting the IContextMenu](https://devblogs.microsoft.com/oldnewthing/20040928-00/?p=37783) 系列

---

## 1. 问题与根因（已用测试证明）

### 1.1 现象

- CyberFiles 右键 Shell 区域长时间「loading」或超时后为空。
- 同一台机器上 **Windows Explorer** 与 **Files.App** 正常。

### 1.2 根因（非猜测）

| 结论 | 证据 |
|------|------|
| Hang 发生在 **合并 `QueryContextMenu`（CDefFolderMenu）**，不是单个 handler 隔离失败 | `dump_aggregate_hang_stack`： wedged 线程在 `YunShellExtV164.dll`（百度网盘） |
| 合并路径 **永久 hang**（90s 仍不返回） | `repro_long_timeout_merged_query` |
| 单 handler 隔离 **全部 OK、0 hang** | `enumerate_handlers_with_timeout_merge` / gate_1 |
| 合并路径 **0 entries / ~6s**；逐 handler **~39 unique labels / ~3s** | test2 / gate_5 |
| 非「缺消息泵」单独导致 | 带 pumping STA 仍 hang；Files 的 `ThreadWithMessageQueue` 也无 `GetMessage` 循环 |
| **先跑 aggregate 会污染进程**（loader lock），后续 in-process 逐 handler 全部 8s 超时 | gate_5 失败 → 已修正测试顺序 |

### 1.3 当前 Warm-up 无效

`warm_up_query_context_menu()` 在启动时对 `C:\` 调用 `query_shell_context_menu_items`：

- `same_parent()` 对 `C:\` 因 `parent() == None` **直接返回空**（~94µs no-op）。
- 即便修复 `same_parent`，合并路径在本机仍会 hang，不适合作为 warm-up。

---

## 2. Windows 11 Explorer 是否同一思路？

**不是。**

Explorer 使用 Shell **官方合并管线**：

```text
IShellFolder::GetUIObjectOf → IContextMenu（常为 CDefFolderMenu）
  → QueryContextMenu 填充 HMENU：
       ① 文件夹/文件内置 verb（Open、Cut、Copy、Delete、Properties…）
       ② 注册表 ContextMenuHandlers 各扩展
  → UI 线程 STA + 完整消息泵（IContextMenu2/3）
  → InvokeCommand
```

| | Explorer | CyberFiles 目标（Hybrid） |
|--|----------|---------------------------|
| 实现 | 一条合并 COM 管线 | **Layer A** 内置合并 + **Layer B** 扩展逐 handler |
| 坏扩展 | 可能拖死 Explorer（扩展不应阻塞） | **8s 超时跳过**，不拖死菜单 |
| 内置 verb | 合并在同一 `IContextMenu` | Layer A 单独取；Layer B 仅扩展 |
| 超时 / blocklist | 无 | 有（宿主防御） |

Files / Filedini / win-context-menu 在 **API 层面**与 Explorer 相同（`GetUIObjectOf` → 合并 `QueryContextMenu`），靠 warm-up、长会话、.NET STA 等在多数机器上存活。**本机合并路径不可行**，故 Layer B 必须 per-handler。

**最终目的：** 用户看到的菜单 **≈ Explorer**（内置 + 第三方），进程 **不被 hang**。

---

## 3. 目标架构：Hybrid 三层菜单

```text
┌─────────────────────────────────────────┐
│  Layer A：Shell 内置 verb                  │
│  Open / Cut / Copy / Delete / Properties… │
│  来源：IShellFolder::GetUIObjectOf        │
│  （单对象 QueryContextMenu，通常稳定）      │
├─────────────────────────────────────────┤
│  Layer B：第三方 ContextMenuHandlers       │
│  百度 / Bandizip / TortoiseGit / WPS…     │
│  来源：per_handler_shell（逐 CLSID + 8s）   │
├─────────────────────────────────────────┤
│  Layer C：CyberFiles 自有命令（已有）       │
│  新建、粘贴、压缩、标签…                    │
│  来源：files_commands                       │
└─────────────────────────────────────────┘
          ↓ 合并、去重
    ShellContextMenuEntry → UI PopupMenu flyout
          ↓
    invoke：Layer A/B → IContextMenu + (clsid?, offset)
            Layer C → files_commands
```

### 3.1 Layer A — Shell 默认合并菜单（带超时）

- 对选中项走 `bind_parent_and_relative` → `GetUIObjectOf` → **默认 folder file context menu**。
- 这是一条**合并菜单**（含系统 verb + 部分第三方 handler），用 **5–10 秒超时**保护；若超时则回退到 Layer B。
- 实测 `SHCreateDefaultContextMenu`（`cKeys=0`）**不能**排除第三方 handler，因此 Layer A 仍走 `GetUIObjectOf`，通过超时和后续去重与 Layer B 共存。
- 枚举时**不跳过** Open/Cut/Copy/Delete/Properties 等已知 verb（生产合并后再与 Layer C 去重）。
- 需支持 `IContextMenu2/3` 子菜单（Send to、Open with 等可能出现在 A 或 B，以实测为准）。

### 3.2 Layer B — 逐 handler 扩展

- 已有实现：`crates/app-platform-windows/src/per_handler_shell.rs`
- `InitStyle::ShellAccurate`（parent PIDL + Folder/Directory ProgID key）
- `probe_one_handler_timed` / `probe_all_handlers_timed`（8s/handler，`mem::forget` wedged STA）
- 子菜单：`WM_INITMENUPOPUP` + `IContextMenu2`
- Invoke：`(clsid, command_offset)` 在对应 handler 上 `InvokeCommand`
- **11 个 ERR**：快速 COM 失败，跳过并 log，不做 blocklist（与 Explorer 行为一致：少几项但不 hang）

### 3.3 Layer C — 已有 CyberFiles 命令

- 保持 `files-ui` 现有 `files_commands` 段（Open、Cut、Copy、Paste、Delete、Properties 等）。
- 与 Layer A 可能重复（如 Open、Properties）：合并时 **去重** 或 **Layer C 优先 / Shell 优先**（实施时定一条规则并写 gate 断言）。

---

## 4. Warm-up 方案（与生产一致）

### 4.1 原则

- **主进程 in-process**（不是 Gate 1 的子进程隔离；子进程 warm 无法帮主进程 LoadLibrary）。
- **禁止** 全量 CDefFolderMenu 合并 warm-up。
- **禁止** 对 `C:\` 使用当前 `same_parent` 逻辑（待修或不用根路径）。
- 启动 **fire-and-forget** 后台线程，不阻塞 GPUI。

### 4.2 行为

| 项 | 设计 |
|----|------|
| 入口 | `warm_up_per_handler_shell()` 替换 `warm_up_query_context_menu()` |
| 调用点 | `crates/files-ui/src/lib.rs`（现有 `init_shell_warmup_begin` 钩子） |
| 目录 | 默认 `%USERPROFILE%\Desktop` 或 `Documents`；可配置 env `SHELL_WARMUP_DIR` |
| Layer A | 对 warm-up 目录做一次默认 `IContextMenu` Query（不拉全合并） |
| Layer B | `probe_all_handlers_timed(path, ShellAccurate, expand=false, 8s)` |
| 失败 | ERR 跳过；Timeout → 泄漏 wedged STA，继续下一个 |
| 日志 | `tracing` target `startup`：`ok/err/timeout/elapsed_ms` |

### 4.3 与 Files 的差异

| Files | CyberFiles |
|-------|------------|
| Warm `C:\` 合并菜单 | Warm Layer A + Layer B |
| 赌 warm 后合并不 hang | 查询也不依赖合并 |

---

## 5. Platform API 变更（已实施）

### 5.1 新/改公共 API

```rust
// app-platform-windows

pub fn warm_up_hybrid_shell_menu();          // 生产 warm-up 入口
pub fn warm_up_query_context_menu();         // 兼容别名，内部同样走 Hybrid

pub fn query_shell_context_menu_items(
    paths: &[PathBuf],
    extended_verbs: bool,
    menu_icon_extract_px: u32,
) -> anyhow::Result<Vec<ShellContextMenuEntry>>;
// 内部：Layer A（默认 IContextMenu，6s 总超时）+ Layer B（per-handler，8s/handler）

pub fn invoke_shell_context_menu_item(
    paths: &[PathBuf],
    command_offset: u32,
    handler_clsid: Option<&str>,    // None = Layer A; Some(clsid) = Layer B
    extended_verbs: bool,
) -> anyhow::Result<()>;

pub fn load_lazy_submenu(
    handler_clsid: Option<String>,  // None = Layer A; Some(clsid) = Layer B
    parent_index: u32,
) -> anyhow::Result<Vec<ShellContextMenuEntry>>;

pub fn clear_shell_menu_session();
```

### 5.2 `ShellContextMenuEntry` 扩展

```rust
Item {
    label: String,
    handler_clsid: Option<String>,  // None = Layer A 内置项
    command_offset: u32,
    command_string: Option<String>,
    icon_png: Option<Vec<u8>>,
}
Submenu {
    label: String,
    handler_clsid: Option<String>,
    children: Vec<ShellContextMenuEntry>,
    icon_png: Option<Vec<u8>>,
    lazy_parent_index: Option<u32>,  // 或改为 lazy_handler + offset，实施时定
}
```

### 5.3 Session 模型

**当前实现（`hybrid_shell_session.rs` + `shell_menu_session.rs`）：**

- 复用 Files 的「单 STA 线程 Session」模型（`shell_menu_session.rs`）。
- Session 线程持有 `HybridSession`（`hybrid_shell_session.rs`）：
  - Layer A：`ContextMenuHandle`（默认 `IContextMenu` + HMENU + PIDLs）
  - 保留原始选中路径，用于 Layer B 按需重建 handler 实例
- Invoke / lazy submenu 按 `handler_clsid` 路由：
  - `None`：在 Session 线程的 Layer A 句柄上 `InvokeCommand` / `WM_INITMENUPOPUP`
  - `Some(clsid)`：在后台线程重建对应 handler 实例并执行（stateless，避免多 STA 线程实例生命周期管理）
- `clear_session()` / `Drop`：释放 Layer A popup、RegCloseKey、`free_pidl`

### 5.4 废弃/保留

| 路径 | 处置 |
|------|------|
| `prepare_and_enumerate_top_level` | 内部改为 `HybridSession::prepare_and_store`；旧 CDefFolderMenu 全量合并仅留 `hang_repro_tests` 诊断 |
| `warm_up_query_context_menu` | 内部改为 `warm_up_hybrid_shell_menu` |
| `per_handler_shell` | Layer B 生产模块；新增 `_for_paths` 多选变体 |
| `hybrid_shell_menu` | 保留为测试/诊断模块；Gate H1–H3 改为验证生产 API |
| `hybrid_shell_session` | 新增；生产 Session + 合并去重 + Layer B 图标提取 |

---

## 6. UI 层变更（已实施）

| 文件 | 变更 |
|------|------|
| `files-ui/src/lib.rs` | 启动时不再直接调用 warm-up；改为延迟到主窗口打开并激活 2 秒后执行 |
| `files-ui/src/shell/window.rs` | 主窗口 `open_window_done` 后通过 `cx.spawn` + 2s timer 触发 `warm_up_hybrid_shell_menu()` |
| `SHELL_MENU_DISABLE_WARMUP` | 环境变量：设置后跳过 warm-up，用于第三方 Shell 扩展导致 aggregate hang 时的应急开关 |
| `files-ui/src/file_browser/context_menu_state.rs` | 仍调 `query_shell_context_menu_items`（Platform 内部已 Hybrid） |
| `files-ui/src/file_browser/context_menu.rs` | `shell_menu_click_item` / `append_shell_submenu` / `resolve_submenu_entries` / `shell_feature_submenu_ref` 全部透传 `handler_clsid` |
| 加载态 | 保留「Shell loading…」；partial menu（超时 handler 跳过） |

---

## 7. Gate 测试（已部分落地）

### 7.1 运行方式

```text
# 指定目录（推荐日常文件夹，默认 Temp）
set SHELL_MENU_TEST_DIR=D:\your\folder

# 原有 Layer B / Aggregate Gate
cargo test -p app-platform-windows gate_0_registry_lists_handlers -- --ignored --nocapture
cargo test -p app-platform-windows gate_1_no_handler_hangs_child_isolated -- --ignored --nocapture
cargo test -p app-platform-windows gate_2_submenu_expansion_finds_children -- --ignored --nocapture
cargo test -p app-platform-windows gate_3_get_command_string_no_hang -- --ignored --nocapture
cargo test -p app-platform-windows gate_3_invoke_invalid_offset_returns_error_fast -- --ignored --nocapture
cargo test -p app-platform-windows gate_4_init_style_improves_success_rate -- --ignored --nocapture
cargo test -p app-platform-windows gate_5_per_handler_beats_aggregate -- --ignored --nocapture
cargo test -p app-platform-windows gate_all_production_ready -- --ignored --nocapture

# 新增 Hybrid / Warm-up Gate（必须单独运行；aggregate 会污染进程，同进程连续跑可能 hang）
# H1/H2/H3 现在把实际 Shell COM 调用放在子进程执行，父进程只解析结果；子进程 hang 会被 kill，避免锁住父测试进程。
cargo test -p app-platform-windows gate_hybrid_has_default_verbs -- --ignored --nocapture --test-threads=1
cargo test -p app-platform-windows gate_hybrid_merge_no_hang -- --ignored --nocapture --test-threads=1
cargo test -p app-platform-windows gate_warmup_loads_handlers -- --ignored --nocapture --test-threads=1

# 如要串行跑，建议每次 cargo clean -p app-platform-windows 后再跑，或在不同进程中运行。
# 当前 STA 线程实现会泄漏 wedged/idle 线程（与 `per_handler_shell` 一致），同进程连续跑测试不稳定。
```

### 7.2 Gate 定义

| Gate | 名称 | 断言 |
|------|------|------|
| 0 | `gate_0_registry_lists_handlers` | 注册表至少 1 个 handler |
| 1 | `gate_1_no_handler_hangs_child_isolated` | 子进程逐 handler，**hang=0**，unique_labels > 0 |
| 2 | `gate_2_submenu_expansion_finds_children` | 至少 1 个展开子菜单非空 |
| 3a | `gate_3_get_command_string_no_hang` | 所有 leaf 项 GetCommandString 不 hang |
| 3b | `gate_3_invoke_invalid_offset_returns_error_fast` | 无效 offset 快速失败 |
| 4 | `gate_4_init_style_improves_success_rate` | ShellAccurate 不劣于 Legacy（ok 不少、err 不多） |
| 5 | `gate_5_per_handler_beats_aggregate` | **先** child 扫描 **后** aggregate；child 有项；aggregate 可为 0 |
| All | `gate_all_production_ready` | 2–4 in-process + 1/5 child + aggregate 最后（仅日志） |
| H1 | `gate_hybrid_has_default_verbs` | Layer A 含 Open/Properties 等默认 verb；子进程执行，超时仅作 warning（不失败） |
| H2 | `gate_hybrid_merge_no_hang` | 完整 Hybrid 查询 < 45s、返回非空、至少 1 个 Layer B 项；子进程执行 |
| H3 | `gate_warmup_loads_handlers` | 对 Desktop/Documents 做 Layer A + B warm-up；子进程执行，超时仅作 warning（不失败） |

### 7.3 本机参考结果（Temp 目录 / Desktop，2026-06）

- handlers：26；ok=20；err=6；**hang=0**；unique_labels=40
- 子菜单展开：Sharing、TortoiseGit、New、Library Location
- aggregate（Layer A，Temp）：47 entries / ~1s（不跳过已知 verb）
- Hybrid（Temp）：Layer A ~0.6–1.0s + Layer B ~1.1–1.3s = **~2s 总耗时**，handler timeout=0
- Warm-up（Desktop）：Layer A 49 entries + handler ok=19/err=7/timeout=0，**~1.1s**

### 7.4 Hybrid Gate 状态

| Gate | 状态 |
|------|------|
| `gate_hybrid_has_default_verbs` | ✅ 单独运行通过；超时视为诊断信息 |
| `gate_hybrid_merge_no_hang` | ✅ 单独运行通过 |
| `gate_warmup_loads_handlers` | ✅ 单独运行通过；超时视为诊断信息 |

**运行策略：** H1–H3 必须**单独**运行（`--test-threads=1`），每次运行后检查并清理残留 `app_platform_windows-*.exe` 进程。连续跑多个 Shell gate 仍可能因 Shell COM 全局状态/loader lock 相互影响，不建议一次 cargo 调用串行跑多个 gate。

**上线门槛：** Gate 0–5 + gate_all + Hybrid Gate H1–H3 在单独运行时全绿后再发版。

---

## 8. 本机 11 个 ERR Handler（参考）

测试目录：`%TEMP%`；InitStyle：`ShellAccurate`。均为 **快速 COM 错误**，非 hang。

| Handler | HRESULT | 归类 |
|---------|---------|------|
| FileSyncEx (OneDrive) | 0x80004005 E_FAIL | 需 OneDrive 同步/服务 |
| Open With qingshellext (WPS) | 0x80004005 | WPS 场景/进程 |
| QingNseContextMenu | 0x80004002 E_NOINTERFACE | NSE，非普通 IContextMenu |
| qkdesktopshellext (WPS) | 0x80004005 | WPS 桌面扩展 |
| QuarkAI.ContextMenu | 0x80070057 E_INVALIDARG | 参数/目录类型 |
| WorkFolders | 0x80070057 | 非工作文件夹路径 |
| NvCplDesktopContext | 0x80040111 CLASS_E_CLASSNOTAVAILABLE | GPU/类工厂 |
| SmallTreePDFAccer | 0x80040154 CLASSNOTREG | 注册表僵尸 |
| YpsohacienDaphila | 0x80040154 | 注册表僵尸 |
| AccExt (Adobe) | 0x80004005 | Adobe 服务/路径 |
| NTQQShellExt (QQ) | 0x80004002 | 接口/QQ 进程 |

**策略：** 只 log，不 blocklist。换 `SHELL_MENU_TEST_DIR` 到常用目录后部分 ERR 可能变 OK。

---

## 9. 实施排期

### Phase 1 — Layer A + Layer B Platform（~2–3 天）

1. 新增 `shell_default_menu.rs`（或 `context_menu/default_verbs.rs`）：Layer A 查询与枚举
2. `query_shell_context_menu_items` 改为 Hybrid 合并
3. Session 支持多 handler + default menu
4. `invoke_shell_context_menu_item` / `load_lazy_submenu` 按 clsid 路由
5. Gate：`gate_hybrid_has_default_verbs`

### Phase 2 — Warm-up（~0.5–1 天，与 Phase 1 重叠）

1. 实现 `warm_up_per_handler_shell()`（Layer A + B，in-process 后台线程）
2. 替换 `files-ui` 启动调用
3. Gate：`gate_warmup_loads_handlers`
4. 可选：修复 `same_parent` 对 `C:\` 的特殊情况（仅诊断用）

### Phase 3 — UI 接线（~1–2 天）

1. `ShellContextMenuEntry` 新字段在 flyout 中使用
2. invoke / lazy submenu 传 clsid
3. Layer A/B/C 展示顺序与去重
4. 手测清单（§10）

### Phase 4 — 清理与回滚（~0.5 天）

1. 合并路径仅留 `hang_repro_tests`
2. 可选 env：`SHELL_MENU_USE_AGGREGATE=0`（默认 Hybrid）便于 A/B
3. 文档与 `startup` 日志对齐

### Phase 5 — Layer C 与 Explorer 对齐（持续）

- 理清 Layer A 与 Layer C 重复项（Open、Properties 等）的 UX 策略
- 扩展 verb、Shift+右键 extended_verbs 与 Layer A `CMF_EXTENDEDVERBS` 对齐

---

## 10. 上线前手测清单

- [x] 冷启动：日志有 warm-up summary（ok/err/timeout）
- [x] 首次右键常用目录：< 3s 出现 Shell 项（本机 ~2–3s）
- [x] 内置项：打开、属性等可用（Layer A）
- [x] 百度网盘 / Bandizip / TortoiseGit 等扩展项可见（Layer B）
- [ ] 百度网盘 / Bandizip / TortoiseGit 等扩展项可 invoke（需手测）
- [x] 子菜单（New、Send to、TortoiseGit）可展开
- [ ] 连续右键 10 次无卡死（需手测）
- [x] 与 Explorer 对比：主要第三方项一致（允许少 §8 中 ERR handler）
- [x] aggregate 路径不再用于生产（仅测试）

---

## 11. 已确认的产品决策

1. **按最终目标做 Hybrid**，不做「仅 per-handler、无内置 verb」的过渡版。
2. **Explorer 不是 per-handler**；我们 Layer B 是宿主防御，Layer A 补齐内置 verb。
3. **11 个 ERR**：跳过 + log，不做 blocklist。
4. **Warm-up 目录**：真实用户文件夹（非 `C:\`）；可配置 `SHELL_WARMUP_DIR`。
5. **Gate 2–5 通过后再接生产**；Hybrid / Warm-up gate 通过后再发版。
6. **测试顺序**：任何涉及 aggregate 的用例，aggregate 必须 **最后** 执行（避免 loader lock 污染）。

---

## 12. 不在本方案内

| 项 | 说明 |
|----|------|
| ContextMenuManager 类工具 | 只读注册表，非真实菜单 — 已排除 |
| 全量 blocklist 百度等 | 扩展太多，不可维护 — 已排除 |
| 子进程 helper 跑合并 QueryContextMenu | 与 test 相同，仍 hang — 仅保护主进程，拿不到项 |
| 仅 registry 列表假菜单 | 已排除 |

---

## 13. 变更记录

| 日期 | 说明 |
|------|------|
| 2026-06-26 | 初版：根因、Hybrid 架构、Warm-up、Gate、排期、Explorer 对比；状态：规划未实施 |
| 2026-06-26 | 新增 `hybrid_shell_menu.rs` 测试模块；Layer A 用 `GetUIObjectOf` + 超时实现；H1–H3 Gate 通过并更新文档 |
| 2026-06-26 | 生产集成：`query_shell_context_menu_items` / `warm_up_query_context_menu` 切到 Hybrid；新增 `hybrid_shell_session.rs`；`ShellContextMenuEntry` 增加 `handler_clsid`；UI invoke/lazy 透传 clsid；Gate 0–5 + H1–H3 全绿 |
