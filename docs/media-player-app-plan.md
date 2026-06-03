# Cyber Media Player Plan

## 1. Goal

新增一个独立的播放器应用，面向完整的音频/视频播放场景。

约束：

- **不改动现有 `CyberFiles` 的音视频预览逻辑**
- **继续使用 `gpui` + `gpui-component`**
- **继续复用当前已经跑通的 `mpv` 音视频能力**
- **视频仍然使用内嵌原生窗口的方式播放**

新应用信息：

- crate / 目录名前缀：`media-player-`
- 可执行文件名：`cyber_media_player`

## 2. Non-goals

这次不是重做文件管理器内的预览。

明确不做：

- 不替换现有 `files-ui` 的 `InfoPane`
- 不回退到 FFmpeg
- 不把完整播放器直接塞进 `CyberFiles`
- 不做 WebView/HTML 播放器路线

## 3. Product Direction

新应用是一个独立桌面播放器，定位比 `InfoPane` 预览更完整：

- 支持单独打开音频/视频文件
- 支持播放列表
- 支持拖拽 / seek / 暂停 / 停止 / 下一首 / 上一首
- 视频使用当前已经验证过的 `mpv` 嵌入窗口路径
- 音频和视频共用一套播放器状态模型

## 4. Naming And Crates

建议新增两个 crate：

### 4.1 `crates/media-player-app`

职责：

- 程序入口
- 创建主窗口
- 初始化日志 / 设置 / 启动参数
- 设置 `GPUI_DISABLE_DIRECT_COMPOSITION=1`
- 启动 `media-player-ui`

二进制：

- package: `media-player-app`
- bin: `cyber_media_player`

### 4.2 `crates/media-player-ui`

职责：

- 播放器页面和 UI 组件
- 控制条
- 播放列表
- 文件打开交互
- 与 `app-mpv-ffi` 的控制层编排

## 5. Reuse Strategy

继续复用现有已经跑通的部分：

### 5.1 直接复用

- `crates/app-mpv-ffi`
  - `MpvEmbedPlayer`
  - `MpvAudioPlayer`
  - 媒体 probe 能力
  - seek / pause / stop / time-pos / duration

- `gpui-component`
  - `Button`
  - `Slider`
  - `Tab`
  - `List` / `DescriptionList`
  - `Alert`

### 5.2 可迁移参考实现

可从现有代码提炼思路，但不要直接把页面代码整体搬过去：

- `crates/files-ui/src/info_pane.rs`
  - 视频嵌入生命周期
  - 音频 / 视频 seek slider
  - 停止 / 暂停 / 恢复 / 快进快退

- `crates/files-ui/src/audio_player.rs`
  - 音频后台线程控制模型

### 5.3 保持不动

这些原有能力保持不变：

- `CyberFiles` 里的音频预览
- `CyberFiles` 里的视频预览
- `InfoPane` 的现有交互

## 6. Architecture

建议使用四层结构。

### 6.1 App Layer

位置：

- `media-player-app`

职责：

- 启动和关闭
- 主窗口创建
- 命令行参数处理
- 传递初始打开文件

### 6.2 UI Layer

位置：

- `media-player-ui`

职责：

- 播放器页面布局
- 控制条
- 播放列表
- 状态展示

### 6.3 Controller Layer

建议先放在：

- `media-player-ui/src/player_controller.rs`

职责：

- 当前媒体状态
- 播放 / 暂停 / 停止
- seek
- 音量 / 静音
- 播放结束
- 自动连播
- 列表切换

### 6.4 Backend Layer

位置：

- `app-mpv-ffi`

职责：

- libmpv 封装
- 视频内嵌
- 音频播放
- 元数据读取

## 7. Windowing Rules

播放器必须沿用当前已经验证过的嵌入策略：

- Windows 下禁用 `DirectComposition`
- 视频使用原生 `HWND` 子窗口承载 mpv
- 布局变化时同步 `MoveWindow`
- 切页面 / 最小化 / resize 时只隐藏或重定位，不随意销毁播放器

这部分是新 app 最关键的稳定性基础。

## 8. First Version Scope

第一版只做最小可用播放器。

### 8.1 文件能力

- 打开单个音频文件
- 打开单个视频文件
- 从命令行传入文件路径

### 8.2 播放控制

- 播放
- 暂停
- 继续
- 停止
- seek slider
- `-5s / +5s`
- 音量 slider
- 静音

### 8.3 显示内容

- 视频显示区
- 音频占位区
- 文件名
- 当前时间 / 总时长
- 基础元数据

### 8.4 列表能力

- 简单播放列表
- 添加多个文件
- 选中并播放
- 上一首 / 下一首

## 9. Second Version Scope

第一版跑稳后再做：

- 自动连播
- 单曲循环 / 列表循环
- 随机播放
- 记忆播放位置
- 拖入文件
- 字幕支持
- 倍速播放
- 全屏
- 置顶
- 最近播放记录

## 10. UI Layout Suggestion

建议第一版使用三段式布局：

### 10.1 Top Bar

- 打开文件
- 打开文件夹
- 当前媒体标题

### 10.2 Main Area

- 左侧：播放列表
- 右侧：主播放区

### 10.3 Bottom Control Bar

- 播放 / 暂停 / 停止
- 上一首 / 下一首
- 时间显示
- seek slider
- 音量 slider

## 11. State Model

建议统一一套播放状态，不再分音频一套、视频一套页面逻辑。

建议状态至少包含：

- `Idle`
- `Loading`
- `Paused`
- `Playing`
- `Stopped`
- `Ended`
- `Failed`

建议核心字段：

- 当前文件路径
- 当前媒体类型
- 总时长
- 当前时间
- 是否静音
- 音量
- 播放列表
- 当前索引

## 12. Suggested Files

建议第一批新增文件：

```text
crates/
  media-player-app/
    Cargo.toml
    src/main.rs

  media-player-ui/
    Cargo.toml
    src/lib.rs
    src/player_page.rs
    src/player_controller.rs
    src/playlist.rs
    src/player_state.rs
```

## 13. Workspace Changes

需要修改：

- 根 `Cargo.toml`
  - 新增 `crates/media-player-app`
  - 新增 `crates/media-player-ui`
  - 视情况加入 `default-members`

建议：

- `media-player-ui` 依赖 `app-mpv-ffi`
- `media-player-app` 依赖 `media-player-ui`

## 14. Implementation Order

建议按这个顺序推进：

1. 新建 `media-player-app`
2. 新建 `media-player-ui`
3. 跑通空窗口
4. 跑通视频嵌入窗口
5. 跑通音频播放
6. 接统一控制条
7. 接 seek / 音量 / 静音
8. 接播放列表
9. 接命令行打开文件
10. 做样式和交互收尾

## 15. Risks

主要风险：

- 原生视频子窗口在复杂布局下的 resize / 遮挡问题
- 播放列表切换时的状态同步复杂度
- `mpv` 事件轮询与 UI 刷新之间的节奏控制

规避方式：

- 第一版先单窗口、单页面、少动画
- 控制器集中管理状态，不要把播放逻辑散在页面里

## 16. Recommendation

推荐结论：

- **新做独立 app**
- **保留原有 `CyberFiles` 音视频预览不变**
- **继续使用当前已经跑通的 mpv 路线**
- **新目录统一使用 `media-player-` 前缀**
- **新 exe 名字固定为 `cyber_media_player`**

这条路径最稳，也最符合当前仓库已经验证过的技术路线。
