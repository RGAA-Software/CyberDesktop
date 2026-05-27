---
name: cargo-run-verify
description: >-
  Mandatory after every assistant reply in CyberFiles: kill any running cyberfiles.exe,
  then cargo run -p cyberfiles, and monitor the Cursor terminals folder for build/runtime
  errors. Use on every turn in this workspace without exception—even for small answers,
  docs-only edits, or when the user did not ask to run the app.
---

# cargo run 验证（每次回答必做）

## 硬性规则（不可跳过）

**每次回答结束前**必须按顺序执行：

1. **结束已有进程**（在启动新的 `cargo run` 之前）
2. **`cargo run -p cyberfiles`**（workspace 根目录）
3. **监听 terminal**（`terminals/*.txt` 或 `Await`）
4. 在回复末尾写简短 **运行验证**

即使用户未提醒、仅回答问题、或只改了一行代码，也**必须**执行。

## 1. 结束已有进程（必须先做）

在 PowerShell（workspace 根目录）：

```powershell
Set-Location D:\source\CyberFiles   # 或当前 workspace 根

# 结束已运行的 CyberFiles GUI
Get-Process -Name "cyberfiles" -ErrorAction SilentlyContinue | Stop-Process -Force

# 可选：结束仍占用 target\debug\cyberfiles.exe 的残留（按镜像路径）
Get-Process | Where-Object { $_.Path -like "*\CyberFiles\target\*\cyberfiles.exe" } | Stop-Process -Force -ErrorAction SilentlyContinue
```

确认无 `cyberfiles` 后再 `cargo run`，避免旧二进制仍在跑、文件被锁或双实例。

## 2. 启动

```powershell
cargo run -p cyberfiles
```

- GUI 长时间运行：Shell `block_until_ms: 0` 放后台。
- 若刚大改依赖，可先 `cargo check`，但**仍须** `cargo run` 验证启动。

## 3. 监听 terminal

- 读取对应 `terminals/<id>.txt` 或使用 `Await`（`pattern`: `Running \`target|error:|panic`）。
- 关注：`Finished`、`Running ...cyberfiles.exe`、`error:`、`panic`、`thread 'main' panicked`、非零 `exit_code`。
- 编译失败或 panic：修代码后**再次** kill → `cargo run` → 监听，直到启动无 panic 或向用户说明阻塞原因。

## 4. 回复模板

```markdown
### 运行验证
- 已结束旧进程: <是 / 无运行中实例>
- `cargo run -p cyberfiles`: <编译成功运行中 | 编译失败 | panic>
- Terminal: <无异常 | 错误摘要>
```

## 失败处理

| 情况 | 动作 |
|------|------|
| 编译失败 | 修到 `cargo check` 通过 → kill → `cargo run` |
| 运行 panic | 按 stack trace 修复 → kill → `cargo run` |
| 端口/文件锁 | 确认已 kill `cyberfiles`，必要时结束卡住的 `cargo` 子进程 |

## 相关文件

- 规则（always apply）：`.cursor/rules/cargo-run-verify.mdc`
