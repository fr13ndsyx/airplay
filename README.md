# airplay-rs

AirPlay 接收端的 Rust 实现 —— 让 iPhone 能将屏幕镜像到 Windows 电脑。
基于上游 Java 项目 `serezhka/java-airplay`（MIT 许可）的逆向成果重写。

## 特性

- 纯 Rust 实现 AirPlay 镜像协议（Pairing / FairPlay / RTSP）
- GStreamer 硬件解码（Windows D3D11，零拷贝），软解自动回退
- mDNS/Bonjour 服务广播，iPhone 自动发现
- 命令行启动，双击 `start.bat` 即用
- 视频画面在 GStreamer 独立原生窗口渲染，保持原始宽高比与低延迟

## 项目架构

```
airplay-rs/
├── crates/
│   ├── fairplay/          # FairPlay 逆向算法（Phase 1）
│   ├── airplay-protocol/  # 协议层：Pairing / FairPlay / RTSP / 解密器
│   ├── airplay-server/    # 网络层：RTSP/HTTP 控制 + mDNS + 会话管理
│   ├── airplay-player/    # 播放层：GStreamer 音视频管线
│   └── airplay-cli/       # 应用层：CLI 入口 + 状态管理
├── start.bat              # Windows 启动脚本
├── Cargo.toml             # workspace 根配置
└── .cargo/config.toml     # GStreamer 构建环境变量
```

### 分层依赖关系

```
┌─────────────────────────────────────────────┐
│           airplay-cli (应用层)               │
│   main.rs · server_task · status            │
└───────────────┬─────────────────────────────┘
                │
      ┌─────────┴─────────┐
      ▼                   ▼
┌───────────┐      ┌─────────────┐
│ airplay-  │      │ airplay-    │
│ server    │      │ player      │
│ (网络层)   │      │ (播放层)    │
└─────┬─────┘      └──────┬──────┘
      │                   │
      ▼                   │
┌─────────────────┐       │
│ airplay-protocol│◄──────┘
│ (协议层)         │
└──────┬──────────┘
       │
       ▼
┌──────────┐
│ fairplay │
│ (算法层)  │
└──────────┘
```

## 主要模块职责

### 1. `fairplay` — FairPlay 逆向算法

FairPlay DRM 的逆向实现。严格 1:1 对应 Java 原实现，不做"优化"。

| 模块 | 职责 |
|---|---|
| [`omg_hax`](crates/fairplay/src/omg_hax.rs) | 主入口，组装 FairPlay 握手响应 |
| [`modified_md5`](crates/fairplay/src/modified_md5.rs) | 修改版 MD5 哈希 |
| [`sap_hash`](crates/fairplay/src/sap_hash.rs) | SAP 哈希计算 |
| [`hand_garble`](crates/fairplay/src/hand_garble.rs) | 字节混淆/反混淆 |
| [`consts`](crates/fairplay/src/consts.rs) | 常量定义 |
| `tables/` | 10 张置换表（table_s1 ~ table_s10） |

### 2. `airplay-protocol` — 协议层

封装 AirPlay 协议握手、加密、解密。

| 模块 | 职责 |
|---|---|
| [`airplay`](crates/airplay-protocol/src/airplay.rs) | 协议 facade，聚合 Pairing / FairPlay / 解密器 |
| [`pairing`](crates/airplay-protocol/src/pairing.rs) | Ed25519/X25519 配对握手 |
| [`fairplay_setup`](crates/airplay-protocol/src/fairplay_setup.rs) | FairPlay SETUP 交换 |
| [`video_decryptor`](crates/airplay-protocol/src/video_decryptor.rs) | AES-CTR 视频流解密 |
| [`audio_decryptor`](crates/airplay-protocol/src/audio_decryptor.rs) | ALAC/AAC-ELD 音频流解密 |
| [`rtsp`](crates/airplay-protocol/src/rtsp.rs) | RTSP 请求/响应编解码 |
| [`stream_info`](crates/airplay-protocol/src/stream_info.rs) | 音视频流参数（codec、分辨率等） |
| [`plist_util`](crates/airplay-protocol/src/plist_util.rs) | plist 序列化工具 |

### 3. `airplay-server` — 网络层

tokio 异步实现的 AirPlay 服务端。

| 模块 | 职责 |
|---|---|
| [`server`](crates/airplay-server/src/server.rs) | `AirPlayServer` —— 聚合 ControlServer + Bonjour |
| [`control/server`](crates/airplay-server/src/control/server.rs) | RTSP/HTTP 控制通道 TCP 服务 |
| [`control/handler`](crates/airplay-server/src/control/handler.rs) | 请求路由与处理 |
| [`bonjour`](crates/airplay-server/src/bonjour.rs) | mDNS 广播 `_airplay._tcp` + `_raop._tcp` |
| [`session`](crates/airplay-server/src/session.rs) | 按 sessionId 管理客户端会话 |
| [`video/server`](crates/airplay-server/src/video/server.rs) | 视频流 TCP 服务（接收加密 H.264） |
| [`audio/server`](crates/airplay-server/src/audio/server.rs) | 音频流 TCP 服务 |
| [`consumer`](crates/airplay-server/src/consumer.rs) | `AirPlayConsumer` trait —— 业务回调接口 |
| [`config`](crates/airplay-server/src/config.rs) | `AirPlayConfig` —— 服务名/分辨率/帧率/设备 ID |

### 4. `airplay-player` — 播放层

GStreamer-rs 实时音视频播放。

| 模块 | 职责 |
|---|---|
| [`player`](crates/airplay-player/src/player.rs) | `GstPlayer` —— 专用线程 + mpsc 通道 |
| [`video_pipeline`](crates/airplay-player/src/video_pipeline.rs) | 视频管线（D3D11 硬解优先，软解回退） |
| [`audio_pipeline`](crates/airplay-player/src/audio_pipeline.rs) | ALAC / AAC-ELD 音频管线 |
| [`consumer`](crates/airplay-player/src/consumer.rs) | `GstPlayerConsumer` —— 实现 `AirPlayConsumer` |

### 5. `airplay-cli` — 应用层（CLI 入口）

| 模块 | 职责 |
|---|---|
| [`main`](crates/airplay-cli/src/main.rs) | 入口：初始化日志/GstPlayer/runtime，自动启动服务，监听 Ctrl+C |
| [`server_task`](crates/airplay-cli/src/server_task.rs) | 封装 Server + Player 生命周期为 tokio task |
| [`status`](crates/airplay-cli/src/status.rs) | `AppStatus` / `ServerCommand` + watch/mpsc 通道类型 |
| [`status_consumer`](crates/airplay-cli/src/status_consumer.rs) | 包装 Consumer，在连接/断开时广播状态 |

## 关键类与函数

### `AirPlayServer`（[server.rs](crates/airplay-server/src/server.rs)）
```rust
pub struct AirPlayServer { /* config + control_server + bonjour */ }

impl AirPlayServer {
    pub fn new(config: AirPlayConfig, consumer: Arc<dyn AirPlayConsumer>) -> Self;
    pub async fn start(&mut self) -> Result<u16>;   // 启动并返回端口
    pub fn stop(&mut self);                          // 停止服务
}
```

### `AirPlayConsumer` trait（[consumer.rs](crates/airplay-server/src/consumer.rs)）
业务方实现此接口接收解密后的音视频数据：
```rust
#[async_trait]
pub trait AirPlayConsumer: Send + Sync {
    async fn on_video_format(&self, info: VideoStreamInfo);  // 收到视频格式
    async fn on_video(&self, bytes: &[u8]);                  // 收到一帧 H.264
    async fn on_audio_format(&self, info: AudioStreamInfo);  // 收到音频格式
    async fn on_audio(&self, bytes: &[u8]);                  // 收到音频数据
    async fn on_video_src_disconnect(&self);                 // 视频源断开
    // ...
}
```

### `GstPlayer`（[player.rs](crates/airplay-player/src/player.rs)）
在专用线程运行 GStreamer pipeline，tokio 侧通过 channel 推送数据：
```rust
impl GstPlayer {
    pub fn new() -> Result<Self>;
    pub fn start_video(&self) -> Result<()>;          // 管线切到 Playing
    pub fn push_video(&self, data: Vec<u8>) -> Result<()>;  // 推送 H.264 帧
    pub fn set_volume(&self, vol: f32) -> Result<()>;
    pub fn consumer(&self) -> GstPlayerConsumer;      // 关联的 Consumer 实现
}
```

### `AirPlayConfig`（[config.rs](crates/airplay-server/src/config.rs)）
```rust
pub struct AirPlayConfig {
    pub server_name: String,   // mDNS 服务名，默认 "airplay-rs-mirror"
    pub width: u32,            // 1920
    pub height: u32,           // 1080
    pub fps: u32,              // 60
    pub device_id: String,     // MAC 格式设备标识
}
```

### `Session` / `SessionManager`（[session.rs](crates/airplay-server/src/session.rs)）
按 `sessionId` 懒创建会话，持有协议状态与子服务器：
```rust
pub struct Session {
    pub id: String,
    pub airplay: AirPlay,                    // 协议 facade
    pub video_server: Option<VideoServer>,   // SETUP 时创建
    pub audio_server: Option<AudioServer>,
}
```

## 依赖关系

### Workspace 核心依赖

| 依赖 | 版本 | 用途 |
|---|---|---|
| `tokio` | 1 (full) | 异步运行时 |
| `gstreamer` / `-app` / `-video` | 0.21 (v1_22) | 音视频解码渲染 |
| `mdns-sd` | 0.10 | mDNS 服务广播 |
| `ed25519-dalek` / `x25519-dalek` | 2 | 配对握手加密 |
| `aes` / `ctr` / `cbc` | 0.8 / 0.9 / 0.1 | 流解密 |
| `plist` | 1 | AirPlay 协议序列化 |
| `httparse` | 1.8 | RTSP/HTTP 解析 |
| `tracing` | 0.1 | 结构化日志 |

### Crate 间依赖

```
airplay-cli ──→ airplay-server ──→ airplay-protocol ──→ fairplay
            └─→ airplay-player ──→ airplay-protocol
            └─→ airplay-protocol
```

## 运行方式

### 环境要求

- **Rust**：stable 工具链（见 [rust-toolchain.toml](rust-toolchain.toml)）
- **GStreamer**：1.22+ MSVC x86_64 版本（含 d3d11 插件）
  - 安装后设置环境变量（见 [.cargo/config.toml](.cargo/config.toml)）：
    ```
    PKG_CONFIG_PATH = D:\gstreamer\1.0\msvc_x86_64\lib
    GSTREAMER_1_0_ROOT_MSVC_X86_64 = D:\gstreamer\1.0\msvc_x86_64\
    ```
- **Windows**：10/11（D3D11 硬解需要）

### 编译

```shell
# debug 版本（编译快，体积大）
cargo build

# release 版本（编译慢，体积小，推荐运行用）
cargo build --release
```

### 启动

**方式一：双击启动脚本（推荐新手）**

双击 [start.bat](start.bat)，自动寻找并运行已编译的二进制。

**方式二：命令行运行**

```shell
# release 版本
cargo run --release

# 或直接运行二进制
.\target\release\airplay-cli.exe

# 开启 debug 日志
$env:RUST_LOG="debug"; cargo run --release
```

### 使用流程

1. 确保电脑与 iPhone 连接同一 Wi-Fi
2. 启动程序（双击 `start.bat` 或命令行运行）
3. iPhone 打开「控制中心」→「屏幕镜像」
4. 选择名为 `airplay-rs-mirror` 的设备
5. 投屏画面在 GStreamer 弹出的独立窗口中显示
6. 按 `Ctrl+C` 退出程序

### 日志级别

通过 `RUST_LOG` 环境变量控制：

| 值 | 说明 |
|---|---|
| `error` | 仅错误 |
| `warn` | 警告 + 错误 |
| `info` | 一般信息（默认） |
| `debug` | 调试细节 |
| `trace` | 全部细节 |

## 测试

```shell
# 全部测试
cargo test

# 按模块测试
cargo test -p fairplay            # FairPlay 算法对比测试
cargo test -p airplay-protocol    # 协议层测试（含 fixtures 回放）
cargo test -p airplay-server      # 网络层测试
cargo test -p airplay-cli         # 状态/命令通道测试
```

协议层测试使用 [fixtures](crates/airplay-protocol/tests/fixtures/one_mirroring_app/) 目录下的真实 AirPlay 会话抓包回放。

## 清理编译产物

Rust 编译产生的 `target/` 目录可能占用数 GB 空间。清理方法：

```shell
# 清理整个 target（释放全部编译产物）
cargo clean

# 仅清理增量缓存（保留 deps，约省一半）
Remove-Item -Recurse -Force target\debug\incremental
```

`target/` 已在 [.gitignore](.gitignore) 中忽略，不会进入版本控制。

## 致谢

本项目基于 `serezhka/java-airplay` 的 FairPlay 逆向成果（MIT 许可）。
`crates/fairplay/` 模块保留原作者的逆向注释，仅做语言翻译。

## 许可证

MIT
