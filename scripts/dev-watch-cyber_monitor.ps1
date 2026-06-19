# 自动监视 monitor-app 源码变化，保存后杀死旧进程并重新编译运行 cyber_monitor 客户端
# 需要先安装 cargo-watch: cargo install cargo-watch
param(
    [string]$Package = "monitor-app",
    [string]$Bin = "cyber_monitor",
    [string]$WatchPath = "crates/monitor-app"
)

Set-Location $PSScriptRoot/..

cargo watch -w $WatchPath -x "run -p $Package --bin $Bin"
