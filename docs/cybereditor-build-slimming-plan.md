# CyberEditor 编译精简执行方案（逐步落地）

目标：把 `cybereditor` 从“完整 IDE 依赖图”收敛到“文本编辑器依赖图”，优先缩短本地增量编译时间与冷编译时间。

范围：仅针对 `cybereditor` 路径；不破坏 `cyberfiles` 主应用现有能力。

## 战略结论（2026-05-28）— 必读

**2026-05-28 更新：** 已物理删除 `crates/editor/`（vendored Zed 栈）与 `crates/cyber-editor-engine/`；`cybereditor` / `cyberfiles` 仅依赖 GPUI + gpui-component。

---

### 为什么「在 Zed 栈上打 feature」达不到 Notepad++ 级体积

本地实测（debug，`target/debug/*.exe`）：

| 目标 | `cargo tree` 行数（normal） | 典型体积 |
|------|---------------------------|----------|
| `cyberfiles`（`full-app`，无 `zed-engine`） | ~1215 | ~60 MB |
| `cybereditor`（`zed-engine`，当前默认） | ~2548 | **~194 MB** |

`cybereditor` 反而比主程序**更大**：因为 `zed-engine` 在 GPUI 壳之外又链入了整条 `editor → project → workspace → language → grammars` 路径，并经由 `tree-sitter` 带入 **wasmtime**（`cargo tree -i wasmtime` 可复现）。  
关闭 LSP 进程、去掉 DAP/extension 只减少**运行时**行为，**几乎不减少**已链接进二进制的 IDE 代码与 Wasm 运行时。

结论：**Notepad++ 类「小、快、依赖少」与「继续裁剪 vendored Zed `editor` crate」是两条产品路线。** 在现有架构上继续 Phase 4 模块 `cfg` 可以有编译时间收益，但很难把 exe 从 ~200MB 级打到 ~30MB 级。

### 推荐方案：双产品、双后端（仓库内已有轻量路径）

代码里已经存在两套 `EditorHost` 后端（`crates/ui/src/cyber_editor/editor_host.rs`）：

| 后端 | Feature | 依赖 | 能力 |
|------|---------|------|------|
| **轻量** | 默认（无 `zed-engine`） | `gpui` + `gpui-component` `InputState::code_editor` | 打开/编辑/保存、行号、折叠、查找替换、**组件内 tree-sitter 高亮** |
| **Zed** | `zed-engine` | + `cyber-editor-engine` + 整条 `editor/project/workspace` | Zed 级编辑体验，体积与 IDE 同级 |

**Phase 0（已完成，2026-05-28）**

1. 已删除 `zed-engine` feature、`zed_backend.rs` 及全部 Zed `editor` 依赖；`cybereditor` 仅使用 `gpui-component` `InputState::code_editor`。
2. 构建：`cargo build -p cyberfiles --bin cybereditor`（无额外 feature）。
3. 实测：debug `cybereditor.exe` **~50 MB**（原 zed 路径 ~194 MB）；`cargo tree` **无** `editor` / `project` / `wasmtime`（~1217 节点，与 `cyberfiles` 壳同级）。

**Phase 1′（1–2 周，增强轻量后端）**

在 `ModelEditorBackend` / `gpui-component` 上补齐 Notepad++ 必需项（不引入 Zed）：

- 大文件与编码（UTF-8 / BOM / 可选 GBK）
- 多标签、会话恢复、外部修改检测
- 查找/替换/正则与「全部替换」性能
- 打印、命令行打开路径、最近文件

**Phase 2′（可选，仅当轻量后端不够时）**

新建 `crates/cyber-notepad-core`（**不**依赖 `crates/editor/editor`）：

- `rope`/`text` 自管 buffer + 按需 **native** tree-sitter grammar（不用 `grammars/load-grammars` 全量嵌入、不用 wasmtime）
- 单一 GPUI `Element` 绘制行块
- 仅当需要超越 `gpui-component` 的性能/行为时再做

**明确不做（对 Notepad++ 目标）**

- 继续在 `editor/notepad` 上拆 `ide-lsp` / `collab-client` 指望显著缩小 exe（收益上限低，工程量大）。
- 把 `cybereditor` 与 `cyberfiles` 绑在同一「必须 Zed」的发布物里。

### 产品边界（写死，避免再次走偏）

```text
cyberfiles  = 完整 IDE（Zed 栈、LSP、工程、AI…）
cybereditor = 高性能文本编辑器（默认 gpui-component 后端；Zed 后端仅 opt-in）
```

### 旧 Phase 1–5 的处理

- **冻结**「在 `zed-engine` 默认开启前提下裁剪 Zed 依赖图」为主线。
- 原 Phase 1（`ui-editor` 拆分）仍可做，但服务于**轻量后端**的壳，而不是继续喂 Zed。
- 原 Phase 4–5（`editor` feature 化、语言包）留给 **cyberfiles / `--features zed-engine`**，不再 blocking `cybereditor` 瘦身。

---

## 当前进度（2026-05-28）

- 清理（2026-05-28，已完成）
  - 删除 `editor` 的 `network-stack` feature 及 `dap`/`telemetry` 可选依赖；移除相关 `#[cfg]` 与 telemetry 调用。
  - 删除 `cyber-editor-engine` 空 feature（`full-editor` / `minimal-editor`）及无用 `init_full`。
  - `cybereditor` 路径关闭 `project-remote-debug`（`editor` 不再经 `network-stack` 或 `project/remote-debug` 拉 DAP）；`session_disabled` 补齐 `SessionEvent` / `EventEmitter` / `any_stopped_thread`。
  - 根 workspace 与 `editor` crate 的 `default` features 为空；`cybereditor` 仅显式 `project-integration`。

- 快速修复（2026-05-28）
  - `cyber-editor-engine` 不再通过 `editor` 默认 feature 拉起 `network-stack`（此前 feature 并集导致 `editor -> dap/telemetry` 仍进 `cybereditor` 链接图）。

- Phase 1（进行中，约 70%）
  - 已完成：
    - 新增 `crates/ui-editor`，`cybereditor` 已切到该入口。
    - `crates/app` 引入 `full-app` / `zed-engine` 分流，`cyberfiles` 与 `cybereditor` 路径解耦。
    - `cyberfiles-ui` 增加 `full-app` / `editor-shell` 特性，并完成 shell 双路径（full-app 与 editor-shell）初始化。
    - 修复回归：`find / replace / replace all` 相关弹窗、菜单链路恢复可用。
    - `theme` 中仅主应用设置链路使用的接口已加 `#[cfg(feature = "full-app")]`，继续压缩 `editor-shell` 编译面。
  - 进行中：
    - 继续将仅主应用使用的模块放入 `#[cfg(feature = "full-app")]`，压缩 `editor-shell` 编译面。
  - 未完成：
    - `ui-editor` 仍依赖 `cyberfiles-ui`，尚未彻底切离；`cybereditor` 仍会编译到部分 `crates/ui`。

- Phase 2（已开始，预处理）
  - 已完成：
    - 梳理 `crates/ui/src/cyber_editor/zed_backend.rs` 中 `project::search::SearchQuery` 与 `workspace::searchable::Direction` 的实际调用面，确认替换边界集中在查找/替换路径。
    - 已将 `workspace::searchable::Direction` 替换为本地 `FindDirection`。
    - 已将 `project::search::SearchQuery` 替换为 `zed_backend` 内部轻量 `FindQuery`，`find / replace / replace all` 保持原有文本匹配语义。
  - 进行中：
    - 校验 `cargo tree -i project/workspace` 的依赖链收敛结果，并继续移除残余间接引用。

- Phase 3（未开始）
  - 已完成：
    - `cyber-editor-engine` 已引入 `full-editor` / `minimal-editor` feature 骨架（默认 `full-editor`），为后续能力裁剪提供开关位。
  - 进行中：
    - `cyberfiles-ui/zed-engine` 已切到 `cyber-editor-engine/minimal-editor`，并在 workspace 依赖层统一关闭 engine 默认特性，开始把开关接入真实构建路径。
    - 将 `minimal-editor` 逐步绑定到实际依赖裁剪。

- Phase 4（进行中）
  - 已完成：
    - `project` 增加 `ide-shell` feature（`extension` / `prettier` 可选）；`cybereditor` 路径不启用，使用 `*_disabled` 桩模块。
    - `snippet_provider` 增加 `extension-host` feature，`cybereditor` 路径不拉 `extension` crate。
    - `cargo tree -p cyberfiles --features zed-engine -i extension` / `-i dap` 已无匹配（DAP/extension 链退出 `cybereditor` 链接图）。
  - 已完成（历史）：
    - `editor` crate 已引入 `network-stack` feature（默认开启），并将 `client/dap/lsp/rpc/telemetry/url` 标记为可选依赖，为后续按构建目标关闭网络链路做准备。
    - `editor` crate 已引入 `project-integration` feature（默认开启），并将 `project/workspace/breadcrumbs` 标记为可选依赖，为后续从 `cybereditor` 路径关闭工程集成能力做准备。
    - `project` crate 已引入 `remote-debug` feature（默认开启），并将 `dap` 设为可选依赖，建立调试链路分层开关位。
    - `workspace` crate 已引入 `collab-runtime` feature（默认开启），并将 `client/telemetry` 设为可选依赖，建立协作运行时分层开关位。
    - `editor` 依赖 `project/workspace` 已切换为显式 feature 透传（并关闭隐式默认特性），避免后续裁剪时被默认特性回拉。
  - 进行中：
    - `cybereditor` 已切到 `editor/notepad` feature（`project-buffer` + `workspace-minimal`），不再走 `project-integration` / `edit-prediction` / `project-remote-debug`。
    - `edit_prediction_types` 的 `client-usage` 仅在 `project-integration` 开启；`hyper` 主链现为 `project` + `editor/notepad` → `client`（`workspace` 已不再默认启用 `collab-runtime`）。
    - `workspace` 可在 `--no-default-features` 下编译：`client` 保留（`Project::local` 类型），`collab-runtime` 仅启用 `telemetry`；`collab_telemetry!` 宏在无遥测时为空操作。
    - `editor/notepad` 的 `workspace` 依赖已去掉 `features = ["collab-runtime"]`。
    - `project` 增加 `collab-client` feature（可选 `client` crate）；无协作时用 `client_shim` + `#[cfg]` 关闭共享/房间 RPC。
    - `workspace` / `editor` 的 `Client`/`UserStore` 类型改由 `project` 重导出；`editor/notepad` 已去掉 `dep:client`。
    - `project/ide-lsp`：关闭时不向语言服务器注册 buffer（语法高亮仍走 `language::Buffer` 的 **tree-sitter**）。
    - `editor/ide-lsp`：完整 IDE 启用；`notepad` 不启用（LSP UI 模块仍编译，但无 LS 进程）。
    - 下一步：`editor` 侧按模块 `cfg(ide-lsp)` 裁剪补全/诊断等编译面。

- Phase 5（保留语言包）
  - **不裁剪** `cyber-editor-engine` 内置语法/语言包（对二进制体积影响小，避免缺高亮）。
  - 高亮路径：tree-sitter 语法树 + `language` 注册表；**不依赖** LSP `textDocument/semanticTokens`。

---

## 基线（先记录，再对比）

在开始改造前，固定一组基线指标：

```powershell
# 冷编译（建议先 clean）
cargo clean -p cyberfiles -p cyberfiles-ui -p cyber-editor-engine
cargo build -p cyberfiles --bin cybereditor --features zed-engine --timings

# 依赖图快照
cargo tree -p cyberfiles --features zed-engine -e normal > .tmp/cybereditor-tree.txt
```

记录项：

- 总构建时间（`Finished ... in ...`）
- `cargo tree` 中是否仍出现 `dap / extension / terminal / prettier / wasmtime`
- 可执行功能回归（打开、编辑、保存、查找）

---

## Phase 1：先把 UI 编译面切开（低风险，高收益）

### 改动目标

- 新建 `crates/ui-editor`，仅保留 `cybereditor` 运行必需模块。
- `cybereditor` 二进制改为依赖 `ui-editor`，不再复用大一统 `cyberfiles-ui`。

### 具体步骤

1. 复制 `crates/ui` 为 `crates/ui-editor`（先不做大重构，先能编译）。
2. 在 `crates/ui-editor/src/lib.rs` 删除与 `cybereditor` 无关模块导出：
   - `file_browser`
   - `main_page`
   - `omnibar`
   - `info_pane`
   - `settings_view`
   - 其他仅 `cyberfiles` 主应用使用的模块
3. `crates/app/src/cybereditor.rs` 改为 `use cyberfiles_ui_editor::{...}`。
4. 根 `Cargo.toml` 增加 workspace member 与依赖项：
   - `crates/ui-editor`
   - `cyberfiles-ui-editor = { path = "crates/ui-editor" }`

### 验证

```powershell
cargo build -p cyberfiles --bin cybereditor --features zed-engine
cargo run -p cyberfiles --bin cybereditor --features zed-engine -- .\README.md
```

### 通过标准

- `cybereditor` 可运行、打开文件、编辑与保存正常。
- `cybereditor` 目标编译不再触发 `ui` 大量无关模块。

### 回滚点

- 仅回退 `cybereditor.rs` 的 crate 引用与 workspace 新增项即可恢复。

---

## Phase 2：移除 `project/workspace` 路径依赖（中风险，核心收益）

### 改动目标

- `cybereditor` 内部不再引用 `project` 和 `workspace` 类型。
- 避免通过 `project -> terminal/extension/prettier/dap` 拉入整条链。

### 具体步骤

1. 审查并替换 `crates/ui/src/cyber_editor/zed_backend.rs` 中：
   - `project::search::SearchQuery`
   - `workspace::searchable::Direction`
2. 在 `cyber_editor` 内实现本地轻量查找模型：
   - `FindDirection`（前/后）
   - `FindQuery`（大小写、整词、regex 可按需分阶段）
3. 所有调用点改为使用本地类型。

### 验证

```powershell
cargo build -p cyberfiles --bin cybereditor --features zed-engine
cargo tree -p cyberfiles --features zed-engine -i project
cargo tree -p cyberfiles --features zed-engine -i workspace
```

### 通过标准

- 查找/下一个/上一个行为与当前一致。
- `project` 与 `workspace` 不再出现在 `cybereditor` 必经链路（或显著收敛）。

### 回滚点

- 保留一份 `zed_backend.rs` 迁移前备份，必要时单文件回退。

---

## Phase 3：给 `cyber-editor-engine` 增加“最小编辑模式”开关（中风险，高收益）

### 改动目标

- `cyber-editor-engine` 提供 `minimal-editor` feature。
- 在该 feature 下只启用文本编辑必要能力（buffer/editor/language/theme/settings）。

### 具体步骤

1. 在 `crates/cyber-editor-engine/Cargo.toml` 增加 features：
   - `default = ["full-editor"]`
   - `full-editor = []`
   - `minimal-editor = []`
2. 清点 `editor` crate 直接使用到的 API 面；准备后续 feature 化切分。
3. `ui-editor` 仅启 `cyber-editor-engine/minimal-editor`。

### 验证

```powershell
cargo build -p cyberfiles --bin cybereditor --features zed-engine
```

### 通过标准

- 行为不回退。
- `cyber-editor-engine` 依赖边界清晰，可继续向下游施压裁剪。

---

## Phase 4：分层 `editor` crate features（高收益，改造最大）

### 改动目标

- 把 `editor` 从硬依赖大集合，拆成可选能力包。

建议分层（初版）：

- `core-editing`：文本编辑核心（必须）
- `project-integration`：项目树/工程搜索
- `lsp`
- `dap`
- `terminal`
- `extensions`
- `ai-assist`

### 具体步骤

1. 在 `crates/editor/editor/Cargo.toml` 引入 feature，逐步把依赖改为 `optional = true`。
2. 第一轮只做“最小可编译”目标：`core-editing` + 当前 `cybereditor` 必需能力。
3. `cyber-editor-engine` 显式启 `editor/core-editing`。

### 验证

```powershell
cargo build -p cyberfiles --bin cybereditor --features zed-engine
cargo tree -p cyberfiles --features zed-engine -i dap
cargo tree -p cyberfiles --features zed-engine -i extension
cargo tree -p cyberfiles --features zed-engine -i terminal
```

### 通过标准

- `cybereditor` 构建链中不再包含 `dap/extension/terminal`（或仅残留可解释边角项）。

---

## Phase 5：语言包（保留，不裁剪）

### 结论

- **保留** `cyber-editor-engine` 内嵌的全部 tree-sitter 语言包；体积收益有限，且易导致打开少见扩展名时无高亮。
- **语法高亮**由 `language` + tree-sitter 提供；与 LSP 语言服务器无关。
- 若将来需要可选语言包，再单独加 `extra-languages` feature，不作为当前 slimming 主线。

### 验证

打开 `.rs` / `.md` / `.json` 等文件，确认高亮正常且进程列表无 `language-server` / `rust-analyzer` 等子进程（`project/ide-lsp` 关闭时）。

---

## 每阶段统一验收清单

```powershell
# 编译
cargo build -p cyberfiles --bin cybereditor --features zed-engine --timings

# 运行
cargo run -p cyberfiles --bin cybereditor --features zed-engine -- .\README.md

# 依赖链检查
cargo tree -p cyberfiles --features zed-engine -i dap
cargo tree -p cyberfiles --features zed-engine -i extension
cargo tree -p cyberfiles --features zed-engine -i terminal
cargo tree -p cyberfiles --features zed-engine -i wasmtime
```

关注变化：

- `cybereditor` 冷编译时间
- 二次增量编译时间（改 1 个 `cyber_editor/*.rs` 文件）
- 重依赖是否退出编译图

---

## 执行顺序（建议，2026-05-28 修订）

```text
Phase 0（默认关闭 zed-engine，启用 ModelEditorBackend）  ← 体积/依赖的主收益
-> Phase 1′（轻量后端功能补齐）
-> Phase 1（ui-editor 与 cyberfiles-ui 解耦，可选）
-> Phase 2′（仅必要时：cyber-notepad-core，无 Zed editor crate）

并行 / 仅 cyberfiles 或 --features zed-engine：
  原 Phase 2–5（Zed 栈 feature 化、LSP cfg）— 不再作为 cybereditor 默认路径
```

说明：

- **先做 Phase 0**；用 `cargo tree` 与 exe 体积对比验证，再投入后续。
- 若 Phase 0 后仍不满意，再上 Phase 2′，不要回到「默认 zed-engine + 继续 cfg」。

