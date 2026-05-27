# 依赖策略（硬性要求）

CyberFiles **不得**通过 Cargo `path` 依赖**本仓库之外**的源码目录（例如 `../gpui-component`）。克隆本仓库后，不应要求开发者额外检出与 CyberFiles 并列的其它仓库才能编译。

---

## 必须遵守

| 规则 | 说明 |
|------|------|
| **禁止仓库外 `path`** | 根 [`Cargo.toml`](../Cargo.toml) 与 `crates/*/Cargo.toml` 中的 `path = "..."` 必须落在 **CyberFiles 仓库根目录内**（仅 `crates/*` 等本仓库路径）。 |
| **第三方 UI 库** | 使用 **Git**（或 crates.io）依赖 [`longbridge/gpui-component`](https://github.com/longbridge/gpui-component)，**禁止** `vendor/` 拷贝整库、**禁止** 修改 fork 后靠同级 `path` 引用。 |
| **库中缺失的能力** | 在 **`crates/ui`**（或合适的本仓库 crate）内实现扩展/包装（例如侧栏文件夹拖放见 `sidebar/menu_with_drop.rs`、`drag::DraggedFilePaths`），并在 PR 中说明；不要向上游仓库打补丁却不合并。 |
| **文档与 Cargo 一致** | 文档示例须与根 `Cargo.toml` 一致；不得写 `../gpui-component` 或 `vendor/gpui-component`。 |

---

## 允许

| 类型 | 说明 |
|------|------|
| **crates.io** | 常规依赖，由 `Cargo.lock` 锁定。 |
| **Git 依赖** | `gpui`、`gpui-component` 等从 Git 拉取（构建需网络，除非已有缓存）。 |
| **行为对照文档** | 可引用外部产品（如 WinUI **Files**）作 parity 说明，**不是**编译依赖。 |

---

## 本仓库实现的 gpui-component 扩展（示例）

| 能力 | 位置 |
|------|------|
| 文件拖放载荷 `DraggedFilePaths` | `crates/ui/src/drag.rs` |
| 侧栏目录项拖放 / 悬停预览 | `crates/ui/src/sidebar/menu_with_drop.rs` |
| 面包屑拖放 | `crates/ui/src/omnibar/breadcrumb_bar.rs`（使用 `DraggedFilePaths`） |
| 面包屑异步下拉菜单 | `crates/ui/src/omnibar/breadcrumb_flyout.rs`（重建 `PopupMenu`，非 `clear_items`） |
| 侧栏中键新标签 | `crates/ui/src/sidebar/menu_with_drop.rs`（`on_mouse_down` 包装） |

查阅上游组件 API / Gallery：单独 clone [gpui-component](https://github.com/longbridge/gpui-component)，在其目录执行 `cargo run -p story`。

---

## 自查

```powershell
# 仓库外 path（应无匹配）
Select-String -Path Cargo.toml, crates\*\Cargo.toml -Pattern 'path\s*=\s*"\.\./'

# UI 库应为 git
Select-String -Path Cargo.toml -Pattern 'gpui-component'
```

---

*最后更新：2026-05-21*
