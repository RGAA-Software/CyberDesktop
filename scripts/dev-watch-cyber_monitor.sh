#!/bin/bash
# 自动监视 monitor-app 源码变化，保存后杀死旧进程并重新编译运行 cyber_monitor 客户端
# 需要先安装 cargo-watch: cargo install cargo-watch

set -e

cd "$(dirname "$0")/.."

cargo watch -w crates/monitor-app -x "run -p monitor-app --bin cyber_monitor"
