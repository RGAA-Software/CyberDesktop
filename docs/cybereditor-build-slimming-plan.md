# CyberEditor 编译精简执行方案（逐步落地）

目标：把 `cybereditor` 从“完整 IDE 依赖图”收敛到“文本编辑器依赖图”，优先缩短本地增量编译时间与冷编译时间。

范围：仅针对 `cybereditor` 路径；不破坏 `cyberfiles` 主应用现有能力。

## 当前进度（2026-05-28）

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
  - 进行中：
    - 准备引入本地轻量方向与查询抽象，逐步替换 `project/workspace` 类型并保持 `find / replace / replace all` 行为一致。

- Phase 3（未开始）
  - `cyber-editor-engine` 还未引入 `minimal-editor` feature。

- Phase 4（未开始）
  - `editor` crate 还未进行能力分层 feature 化。

- Phase 5（未开始）
  - 语言内置集合尚未裁剪。

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

## Phase 5：语言与语法包裁剪（低风险，中收益）

### 改动目标

- 把内置语言集从 20+ 收敛到常用集，剩余做可选。

### 具体步骤

1. 修改 `crates/cyber-editor-engine/src/languages.rs` 中 `EMBEDDED_LANGUAGE_FOLDERS`。
2. 保留最小常用集（建议首批）：
   - `text`
   - `rust`
   - `json`
   - `markdown`
   - `typescript`
   - `javascript`
   - `yaml`
3. 为扩展语言准备 feature（例如 `extra-languages`）。

### 验证

```powershell
cargo build -p cyberfiles --bin cybereditor --features zed-engine
```

### 通过标准

- 常用文本类型高亮正常。
- 构建时间继续下降。

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

## 执行顺序（建议）

```text
Phase 1 (ui-editor 拆分)
-> Phase 2 (去 project/workspace 依赖)
-> Phase 3 (engine 最小模式)
-> Phase 4 (editor feature 化)
-> Phase 5 (语言集裁剪)
```

说明：

- `Phase 1/2` 先做，见效快、风险可控。
- `Phase 4` 是最大收益但工程量最大，放到中后期。

