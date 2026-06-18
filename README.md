# CyberDesktop

GPUI 驱动的桌面应用集合。

## 开发规范

### 代码格式化（强制）

**每次修改完 Rust 源文件后，必须运行 `cargo fmt` 对修改的文件进行格式化，保证代码格式一致。**

推荐做法：

```bash
# 格式化所有文件
cargo fmt --all

# 或者只格式化本次修改的文件
cargo fmt -- <path1> <path2>
```

提交前请确保 `cargo fmt --all` 不会产生新的 diff。

## 常用命令

```bash
# 检查整个工作区
cargo check --workspace

# 运行测试
cargo test --workspace

# 构建 monitor 应用
cargo build -p monitor-app
```

## 主题

`crates/app-assets/themes/` 目录存放 GPUI 主题 JSON。`CyberMonitor` 主题基于 `CyberEditor` 模板、以 `#7548d8` 为主色生成，生成脚本位于 `scripts/gen_cybermonitor_theme.py`。
