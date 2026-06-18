# CyberMonitor 任务管理器化路线图

## 目标

把 CyberMonitor 从“硬件性能遥测面板”升级为一个**类似 Windows 任务管理器、同时保留推送到 Host 能力**的系统监控工具。

- **客户端（cyber_monitor）**：本地采集系统/进程数据，可推送到 Host。
- **Host（cyber_monitor_host）**：接收多台客户端数据，集中展示。

## 当前状态

CyberMonitor 已具备：

- CPU、内存、磁盘容量、网络、传感器、GPU（NVIDIA/AMD）硬件遥测。
- 6 个 Dashboard Tab：总览、CPU/内存、GPU、存储、网络、传感器。
- WebSocket 推送到 Host（`/sys/info`）。
- Host 端多机器列表与远程 Dashboard。
- Windows 托盘图标、单实例、设置面板。

主要不足：

- 没有进程列表。
- 没有服务、启动项、用户会话。
- 磁盘/网络指标偏硬件，缺少读写速率、每进程 IO。
- CPU 当前频率为占位值 0.0。

## 与 TaskExplorer 的差距

| 功能 | CyberMonitor | TaskExplorer |
|------|-------------|--------------|
| 进程列表 | ❌ 无 | ✅ 进程树、过滤、排序 |
| 进程操作 | ❌ 无 | ✅ 结束、暂停、冻结、优先级、亲和性、DLL 注入等 |
| CPU 详细度 | ⚠️ 基础 | ✅ 内核/用户/DPC、上下文切换、中断等 |
| 内存详细度 | ⚠️ 基础 | ✅ Commit、页文件、池、内存列表 |
| 磁盘 IO | ⚠️ 仅容量 | ✅ 每盘读写速率、延迟、队列深度 |
| 网络详细度 | ⚠️ 仅默认网卡 | ✅ 多网卡、Sockets、DNS 缓存、RPC |
| 服务/启动项 | ❌ 无 | ✅ 完整支持 |
| 进程详情页 | ❌ 无 | ✅ Handles/Threads/Modules/Memory/Token 等 |
| 内核级能力 | ❌ 无 | ✅ KSystemInformer 驱动、隐藏进程 |
| 推送到 Host | ✅ 有 | ❌ 无 |

## 分阶段实现计划

### 第一阶段：基础任务管理器能力（优先级最高）

1. **进程列表（Processes Tab）**
   - 扩展 `SysInfo`：增加 `Vec<SysProcessInfo>`。
   - 字段：PID、PPID、名称、CPU%、内存字节数、磁盘读/写字节数、状态、路径。
   - 使用 `sysinfo::System::processes()` 采集。
   - UI：新增“进程”Tab，使用表格展示，支持排序、搜索框。
   - Host 端自动支持（通过 JSON 反序列化）。

2. **修复现有硬件指标**
   - 修复 CPU `current_frequency` 为 0.0 的问题。
   - 磁盘增加读写速率（基于累计字节数差分）。
   - 网络列出所有物理网卡，显示实时速率。

3. **进程操作（右键菜单）**
   - 结束任务（Terminate）。
   - 打开文件位置（Reveal in Explorer）。
   - 查看属性（弹出进程详情窗口，第一阶段可仅展示 General 信息）。

### 第二阶段：Windows 系统管理

4. **服务（Services Tab）**
   - 使用 `windows` crate 调用 Service Control Manager 枚举服务。
   - 显示名称、显示名、状态、启动类型。
   - 支持启动/停止/重启。

5. **启动项（Startup Tab）**
   - 读取注册表 `Run`/`RunOnce`。
   - 读取 `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`。
   - 显示名称、命令、位置、启用状态。

6. **用户（Users Tab）**
   - 使用 `sysinfo` 已提供的用户列表。
   - 显示用户名、登录时间、会话信息。

### 第三阶段：高级进程分析

7. **进程详情页**
   - General、Performance、Threads、Modules、Handles、Network、Environment、Token。
   - 逐步增加，先实现 General + Performance。

8. **更多进程操作**
   - 设置优先级、I/O 优先级、CPU 亲和性。
   - 暂停/恢复进程。
   - 结束进程树。

### 第四阶段：远程 Host 增强

9. **Host 端汇总**
   - 多机进程 Top N 汇总。
   - 离线/在线状态增强。
   - 简单的告警规则（如 CPU > 90%）。

10. **传输增强**
    - WebSocket 鉴权、TLS 支持、断线重连、数据压缩。

## 数据 schema 扩展建议

```rust
pub struct SysProcessInfo {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub command: Vec<String>,
    pub exe_path: Option<String>,
    pub status: ProcessStatus,
    pub cpu_usage: f32,
    pub memory_bytes: u64,
    pub virtual_memory_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_written_bytes: u64,
}

pub struct SysServiceInfo {
    pub name: String,
    pub display_name: String,
    pub status: String,
    pub start_type: String,
}

pub struct SysStartupInfo {
    pub name: String,
    pub command: String,
    pub location: String,
    pub enabled: bool,
}
```

## UI 规划

- 在现有 TabBar 中新增“进程”、“服务”、“启动项”、“用户”Tab。
- 进程列表使用 `gpui_component::Table`。
- 右键菜单使用 `app_ui::PopupMenu` 或 `gpui_component::ContextMenu`。
- 搜索框复用 `gpui_component::Input`。

## 下一步行动

从 **第一阶段第 1 项** 开始：添加进程列表 Tab，先展示基础进程信息，再逐步加入搜索、排序和操作。
