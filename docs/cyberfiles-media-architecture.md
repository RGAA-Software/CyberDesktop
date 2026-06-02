# CyberFiles 媒体预览架构设计

本文是 CyberFiles 后续音频/视频预览与播放能力的**实施设计文档**。目标不是做临时预览，而是在现有代码库中引入一套长期可维护的媒体子系统，统一服务：

- Files `InfoPane` 的音频预览
- Files `InfoPane` 的视频预览
- 元数据探测（时长、编码、分辨率、比特率等）
- 首帧/封面提取
- 播放控制（播放、暂停、停止、seek）
- 音视频同步

本文约束最终实现方案，不讨论占位方案或过渡方案。

---

## 1. 目标

### 1.1 功能目标

- 音频、视频统一使用 `rust-ffmpeg` / `ffmpeg-next` 作为唯一媒体后端。
- 音频、视频共用统一的探测、解复用、解码、时钟、错误处理模型。
- 视频在 Windows 下最终显示为 **GPUI 窗口内的 D3D11 纹理**。
- 视频解码策略固定为：
  - 先尝试硬解码
  - 失败自动回退到软件解码
  - 不向用户暴露“强制硬解/强制软解”切换
- 音视频播放控制必须支持：
  - Play
  - Pause
  - Stop
  - Seek
  - End-of-stream
  - Error recovery

### 1.2 架构目标

- 媒体能力必须独立成明确的模块边界，而不是散落在 `info_pane.rs`、`audio_player.rs` 等页面代码里。
- UI 层只消费媒体状态，不直接感知 FFmpeg API。
- Windows 专属图形/音频设备能力收敛到平台层。
- 后续支持：
  - 更丰富的视频格式
  - 缩略图缓存
  - 文件夹中媒体批量预览
  - 独立媒体窗口 / 全屏

### 1.3 非目标

- 不做浏览器式 `<video>`/WebView2 最终方案。
- 不保留当前音频实现作为长期并行后端。
- 不在第一版引入 Linux/macOS 的完整媒体渲染实现；当前设计先以 Windows 为主。

---

## 2. 当前代码现状

### 2.1 现状摘要

- Files `InfoPane` 已支持：
  - 图片/文本预览
  - 音频预览播放
  - 视频 placeholder
- 当前音频预览相关实现分散在：
  - `crates/files-ui/src/info_pane.rs`
  - `crates/files-ui/src/audio_player.rs`
  - `crates/files-fs/src/audio_metadata.rs`
- 当前视频仅有 `PreviewKind::Video` 分支，但没有真实媒体后端。
- 当前 Windows UI 渲染底层已经在使用 D3D11。

### 2.2 当前问题

- 音频与视频没有统一媒体后端。
- metadata、decode、playback 状态、UI 状态没有清晰边界。
- 当前音频播放实现不适合作为未来视频架构基础。
- 视频预览如果继续走 placeholder/普通 image，将无法支撑最终目标。

---

## 3. 总体方案

### 3.1 核心原则

最终方案分三层：

1. **媒体核心层**
   - 统一封装 FFmpeg
   - 提供探测、解码、时钟、session 管理
2. **Windows 平台层**
   - 音频输出设备
   - D3D11 纹理与视频帧呈现
   - 硬件解码相关平台能力
3. **UI 层**
   - `InfoPane` 媒体面板
   - 播放控制条
   - 状态/错误显示

### 3.2 依赖方向

最终依赖方向应为：

- `files-ui -> app-media`
- `app-media -> app-platform-windows`（仅在 Windows feature/path 上）
- `app-media -> ffmpeg-next`

UI 不得直接调用 FFmpeg。

---

## 4. crate / 模块拆分

## 4.1 新增 crate

建议新增：

- `crates/app-media`

职责：

- FFmpeg 初始化与生命周期管理
- 媒体探测
- session 管理
- 音频/视频解码
- 时钟
- 硬解/软解回退逻辑
- 媒体状态事件分发

### 4.2 `app-media` 模块建议

建议结构：

```text
crates/app-media/src/
  lib.rs
  error.rs
  init.rs
  probe.rs
  source.rs
  metadata.rs
  session.rs
  command.rs
  event.rs
  clock.rs
  demux.rs
  decode/
    mod.rs
    audio.rs
    video.rs
    hardware.rs
    software.rs
  render/
    mod.rs
    audio.rs
    video.rs
  cache.rs
```

### 4.3 `app-platform-windows` 新增模块

建议新增：

- `src/media_audio.rs`
- `src/media_video.rs`
- `src/d3d11_texture.rs`
- `src/d3d11_device.rs`

职责：

- 音频设备输出
- D3D11 纹理创建/更新
- 与硬件视频帧或 CPU RGBA 帧的桥接
- 统一封装 Windows 图形句柄，避免上层 UI 直接接触 Win32 细节

### 4.4 `files-ui` 新增/调整模块

建议新增：

- `src/media_preview.rs`
- `src/video_surface.rs`

并逐步替换：

- `audio_player.rs` 最终被 `app-media` 替代
- `info_pane.rs` 只负责绑定媒体状态和渲染控件

---

## 5. 核心数据结构

### 5.1 媒体源

```rust
pub enum MediaSource {
    File(PathBuf),
}
```

当前仅支持文件路径，后续可扩展 archive entry / stream / temp extraction。

### 5.2 元数据

```rust
pub struct MediaMetadata {
    pub duration: Option<Duration>,
    pub bitrate_kbps: Option<u32>,
    pub file_size: Option<u64>,
    pub audio: Option<AudioMetadata>,
    pub video: Option<VideoMetadata>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

pub struct AudioMetadata {
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

pub struct VideoMetadata {
    pub codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<f32>,
    pub pixel_format: Option<String>,
    pub has_audio: bool,
}
```

### 5.3 session 状态

```rust
pub enum MediaSessionState {
    Idle,
    Probing,
    Ready,
    Playing,
    Paused,
    Seeking,
    Ended,
    Failed(MediaError),
}
```

### 5.4 解码模式

```rust
pub enum VideoDecodeMode {
    Hardware,
    Software,
}
```

注意：这个字段用于内部状态和日志，不是用户设置项。

### 5.5 视频帧

```rust
pub enum VideoFrame {
    D3D11Texture(VideoTextureFrame),
}

pub struct VideoTextureFrame {
    pub width: u32,
    pub height: u32,
    pub pts: Duration,
    pub texture_id: u64,
}
```

这里的 `texture_id` 只是逻辑标识，最终可替换成更适合 GPUI 的纹理句柄包装。

### 5.6 命令与事件

```rust
pub enum MediaCommand {
    Open(MediaSource),
    Play,
    Pause,
    Stop,
    Seek(Duration),
    Close,
}

pub enum MediaEvent {
    Probed(MediaMetadata),
    StateChanged(MediaSessionState),
    PositionUpdated(Duration),
    VideoFrameReady(VideoFrame),
    PosterReady(VideoFrame),
    Error(MediaError),
}
```

---

## 6. 播放生命周期

### 6.1 打开文件

流程：

1. UI 发出 `MediaCommand::Open`
2. `MediaSession` 创建 probe context
3. 读取 container / streams / metadata
4. 选择最佳 audio stream、video stream
5. 视频先尝试硬解初始化
6. 若硬解初始化失败，自动切软解
7. 构建时钟与输出线程
8. 发出 `Probed` + `Ready`

### 6.2 播放

流程：

1. UI 发出 `Play`
2. session 进入 `Playing`
3. demux thread 读取 packet
4. packet 送入 audio/video decode pipeline
5. audio renderer 建立主时钟
6. video renderer 按主时钟提交帧

### 6.3 暂停

流程：

1. session 停止推进时钟
2. audio output 暂停
3. video 保持最后一帧

### 6.4 seek

流程：

1. UI 发出 `Seek(target)`
2. session 切到 `Seeking`
3. demux 清空 packet 队列
4. audio/video decoder flush
5. FFmpeg seek 到目标时间附近
6. 重新开始送包
7. 丢弃 seek 前旧帧
8. 首个稳定帧到达后恢复 `Playing` / `Paused`

### 6.5 关闭

流程：

1. 停止 demux/decode/render 线程
2. 释放 decoder/context/texture/audio device state
3. 释放 FFmpeg 相关对象

---

## 7. 硬解优先 / 软解回退策略

这是本设计的强制要求。

### 7.1 规则

- 每次打开视频时，先尝试硬解码。
- 若硬解路径任何一步失败，则自动回退软件解码。
- 不允许用户手动选择硬解/软解。
- 日志必须记录：
  - 是否走硬解
  - 硬解失败原因
  - 是否成功回退软解

### 7.2 硬解失败触发条件

包括但不限于：

- 硬件设备创建失败
- 当前 GPU/驱动不支持目标 codec
- FFmpeg 硬件像素格式协商失败
- 硬件帧转 D3D11 纹理失败
- seek 后硬件 decoder 恢复失败

### 7.3 回退要求

- 回退必须是自动的，且不中断用户的基本播放体验。
- UI 可以在 debug/log 层记录“已回退到软件解码”，但不弹用户级报错。
- 只有软解也失败时，才进入 `Failed`。

### 7.4 日志示例

```text
[media] opening video: D:\Videos\demo.mp4
[media] hardware decode requested
[media] hardware decode init failed: dxva device unavailable
[media] falling back to software decode
[media] software decode active
```

---

## 8. 线程模型

最终不建议把所有媒体逻辑塞在一个线程里。

### 8.1 推荐线程

- `session control thread`
- `demux thread`
- `audio decode/output thread`
- `video decode/render thread`

### 8.2 线程职责

#### session control thread

- 接收 `MediaCommand`
- 管理 session 状态
- 触发 seek / close / reopen

#### demux thread

- 调 FFmpeg 读 packet
- 按 stream 分发到 audio/video 队列

#### audio decode/output thread

- 解码音频 packet
- 重采样
- 输出到音频设备
- 更新主时钟

#### video decode/render thread

- 解码视频 packet
- 硬解或软解帧转换
- 生成/更新 D3D11 纹理
- 按音频主时钟调度显示

### 8.3 队列

建议：

- `audio_packet_queue`
- `video_packet_queue`
- `video_frame_queue`
- `event_queue`

这些队列必须支持：

- backpressure
- close signal
- seek flush

---

## 9. 时钟与同步

### 9.1 主时钟规则

- 有音频时：音频时钟为主时钟
- 无音频时：视频时钟为主时钟

### 9.2 视频同步规则

视频线程每次拿到帧后，根据 `frame_pts` 与主时钟比较：

- 若帧过早：等待
- 若帧稍晚：立即显示
- 若帧明显过时：丢帧

### 9.3 seek 后同步

seek 后必须：

- flush audio/video decoder
- 丢弃旧 frame
- 重新建立有效基准时间

---

## 10. Windows 渲染方案

### 10.1 最终要求

Windows 视频显示必须以 **D3D11 纹理** 为目标。

不接受以下作为最终实现：

- WebView2 `<video>`
- 系统外部播放器嵌入
- 普通 CPU 图片控件作为正式显示路径

### 10.2 最终渲染分层

- `app-media`
  - 负责生成可显示的视频帧资源
- `app-platform-windows`
  - 负责 D3D11 texture 生命周期
- `files-ui`
  - 负责 `VideoSurface` 这个 GPUI 元素

### 10.3 UI 层职责

`VideoSurface` 只做三件事：

- 持有当前帧句柄/引用
- 请求重绘
- 在 GPUI 中绘制底图

播放按钮、进度条、悬浮 overlay 不应和视频帧上传逻辑耦合。

### 10.4 软解渲染路径

即便最终走 D3D11 显示，软解仍然是：

1. FFmpeg 解码出 software frame
2. 转换到统一像素格式
3. 上传到 D3D11 texture
4. GPUI 显示该纹理

也就是说，**软解 != CPU image 控件**。

### 10.5 硬解渲染路径

理想路径：

1. FFmpeg 使用硬件设备上下文
2. 解码得到硬件帧
3. 转移/映射到可显示 D3D11 资源
4. GPUI 显示对应纹理

---

## 11. UI 集成方案

### 11.1 `InfoPane` 最终职责

`InfoPane` 只负责：

- 创建/销毁 `MediaSession`
- 订阅 `MediaEvent`
- 显示 metadata
- 渲染视频面板和控制层

它不再承担：

- 自己探测 metadata
- 自己实现 audio player
- 自己管理 decode 线程

### 11.2 Audio Preview

Audio 面板展示：

- title
- artist
- album
- codec
- sample rate
- channels
- bitrate
- file size
- transport controls

### 11.3 Video Preview

Video 面板展示：

- 视频纹理面板
- title / codec / resolution / fps / duration
- progress
- transport controls
- error / fallback 状态（日志级别为主）

### 11.4 Poster Frame

视频未播放前：

- session 打开后应尽快提取首帧或接近首帧的可显示帧
- 该帧作为 poster 显示

---

## 12. API 设计建议

### 12.1 对 UI 暴露的主接口

```rust
pub struct MediaController { ... }

impl MediaController {
    pub fn open(&self, source: MediaSource) -> anyhow::Result<()>;
    pub fn play(&self) -> anyhow::Result<()>;
    pub fn pause(&self) -> anyhow::Result<()>;
    pub fn stop(&self) -> anyhow::Result<()>;
    pub fn seek(&self, position: Duration) -> anyhow::Result<()>;
    pub fn close(&self) -> anyhow::Result<()>;
}
```

### 12.2 UI 订阅接口

```rust
pub trait MediaEventSink: Send + Sync {
    fn on_event(&self, event: MediaEvent);
}
```

或者使用内部 channel + UI 轮询更新。

### 12.3 Probe-only 接口

```rust
pub fn probe_media(source: &MediaSource) -> anyhow::Result<MediaMetadata>;
```

用于非播放场景，例如：

- 文件列表 tooltip
- InfoPane 先显示 metadata
- 缩略图索引

---

## 13. 错误处理

### 13.1 错误分类

```rust
pub enum MediaError {
    OpenFailed(String),
    ProbeFailed(String),
    DecoderInitFailed(String),
    HardwareDecodeFailed(String),
    AudioOutputFailed(String),
    VideoRenderFailed(String),
    SeekFailed(String),
    Unsupported(String),
}
```

### 13.2 用户可见策略

- 硬解失败但软解成功：不报用户错误，仅记日志
- 打开失败：显示明确错误
- 不支持格式：显示明确错误
- 音频设备不可用：显示明确错误
- 视频纹理创建失败：显示明确错误

---

## 14. 配置与日志

### 14.1 配置项

当前不建议给用户暴露“硬解开关”。

可考虑只保留内部/实验设置：

- `media_logging_verbose`

若未来需要调试项，可加：

- `media_debug_overlay`

但不能变成用户功能设置。

### 14.2 日志要求

至少记录：

- 打开的媒体路径
- stream 选择结果
- metadata probe 结果
- 是否存在 audio/video stream
- 硬解是否成功
- 回退软解原因
- seek 操作
- end-of-stream
- renderer 错误

---

## 15. 测试策略

### 15.1 单元测试

可测试：

- metadata probe
- stream selection
- duration/bitrate/fps 解析
- fallback decision logic
- state transition logic

### 15.2 集成测试

准备媒体样本：

- `mp4 + h264 + aac`
- `mp4 + h264 only`
- `mkv + h264 + opus`
- `webm`
- 损坏文件
- 无法硬解的编码样本

验证：

- probe 成功
- poster 提取成功
- play/pause/stop/seek 正常
- 硬解失败后自动软解

### 15.3 手动验收

必须手动确认：

- 右侧视频面板真实显示
- 音画同步
- 切换文件后资源释放
- seek 不卡死
- 重复打开关闭不泄漏

---

## 16. 实施顺序

这里的顺序不是过渡方案，而是最终目标的拆解顺序。

1. 新建 `app-media` crate 与基础类型 `[已完成]`
2. 接入 FFmpeg 初始化、probe、metadata `[已完成]`
3. 建立 `MediaSession` 和命令/事件系统 `[已完成，当前仍是基础状态机骨架]`
4. 把现有 audio preview 迁到 `app-media` `[已完成]`
5. 建立视频解码管线 `[进行中：已可 probe 视频 metadata、提取静态 poster，并可输出连续视频帧流]`
6. 建立 D3D11 纹理呈现路径 `[未开始]`
7. 做 `VideoSurface` GPUI 元素 `[未开始]`
8. 接入 `InfoPane` `[进行中：已接入视频 metadata + poster 面板，并有最小可用的播放预览入口]`
9. 实现 seek / clock / fallback logging `[未开始]`
10. 清理旧 `audio_player.rs` 路线 `[未开始]`

---

## 17. 迁移约束

- 旧音频实现不能长期保留为并行正式实现。
- `InfoPane` 中与音频/视频解码强耦合的代码必须逐步移出。
- 最终播放逻辑必须归拢到 `app-media`。
- `files-fs` 保留轻量 probe 辅助可以，但不能变成第二套播放器后端。

---

## 18. 交付标准

当以下条件全部满足时，可认为媒体架构第一阶段完成：

- audio preview 已迁移到 `rust-ffmpeg`
- video preview 可真实播放
- video 显示走 D3D11 纹理
- 硬解优先、失败自动回退软解
- seek 正常
- `InfoPane` 仅承担 UI 绑定
- 日志完整

这才算“媒体子系统接入完成”，不是仅仅“视频能播了”。
