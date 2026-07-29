# AirPlay 接收端 Rust 重构 — 开发流程文档

> 本文档是 `java-airplay` 项目用 Rust 重构的完整开发流程指引。
> 目标读者：项目作者本人（既是开发者也是用户）。
> 编写日期：2026-07-13
> 上游参考：`c:\Users\Administrator\Desktop\airplay\`（Java 版 v1.0.7，MIT 许可）

---

## 0. 文档说明

### 0.1 文档目的

把"用 Rust 重写 java-airplay"这件事拆成**可执行的阶段**与**可验证的产出**，每个阶段都有：

- 输入：依赖的前置工作
- 任务清单：具体要做的事
- 验证标准：跑通什么测试算完成
- 退出条件：满足什么条件可进入下一阶段

### 0.2 总体策略

| 层 | 策略 | 说明 |
|----|------|------|
| FairPlay 核心（逆向算法） | **逐字翻译** | 算法是逆向产物，改一行即坏 |
| 协议层（Pairing/RTSP/解密器） | **翻译 + 类型强化** | 用 Rust 类型系统替代 `byte[]` |
| 网络层（Netty handlers） | **重新设计** | tokio 比 Netty 简洁 |
| 播放器层 | **重新设计** | GStreamer-rs 直接对接 |
| GUI / 启动 / 配置 | **完全新写** | 丢弃 Spring Boot |

### 0.3 不在本文档范围内

- iOS / macOS 客户端逆向（继续复用上游成果）
- AirPlay 3 新特性（先做 AirPlay 2 镜像，新特性后续迭代）
- 嵌入式设备移植（先保证桌面平台）

---

## 1. 项目目标与定位

### 1.1 一句话定位

> 一个**单二进制、双击即用**的 AirPlay 接收端，让 iPhone / iPad / Mac 能把屏幕投到任意 Windows / macOS / Linux 电脑上，画质、延迟、稳定性对标 Apple TV。

### 1.2 必须达成

- ✅ AirPlay **镜像**功能可用（视频 + 音频）
- ✅ 单二进制部署，无运行时依赖（GStreamer 仍需系统安装）
- ✅ 系统托盘常驻 + 视频窗口自动弹出
- ✅ Windows / macOS / Linux 三平台
- ✅ FairPlay 解密与原 Java 版字节一致

### 1.3 可选达成

- HLS / YouTube 投屏（上游部分 TODO，可顺带补全）
- 投屏录制（dump H264）
- 多设备同时投屏
- 开机自启
- 全局快捷键

### 1.4 不做

- ❌ AirPlay 发送端（上游 `client` 模块不做迁移）
- ❌ 自己实现音视频解码（直接用 GStreamer）
- ❌ 重写 FairPlay 算法（必须忠实翻译）

---

## 2. 技术栈选型（最终决策）

### 2.1 决策表

| 维度 | 选型 | 替代 Java |
|------|------|-----------|
| 语言 | Rust 1.75+（2021 edition） | Java 17 |
| 异步运行时 | `tokio` 1.x（multi-thread） | Netty EventLoop |
| 字节缓冲 | `bytes` 1.x | Netty ByteBuf |
| mDNS | `mdns-sd` 0.10+ | jmdns 3.5.8 |
| AES | `aes` 0.8 | JDK Cipher |
| CTR / CBC 模式 | `ctr` 0.9 / `cbc` 0.1 | 同上 |
| Ed25519 | `ed25519-dalek` 2.x | net.i2p.crypto.eddsa |
| X25519 ECDH | `x25519-dalek` 2.x | curve25519-java |
| SHA-512 | `sha2` 0.10 | JDK MessageDigest |
| Plist | `plist` 1.x | dd-plist 1.26 |
| HLS/M3U8 | `m3u8-rs` 5.x | m3u8-parser 0.24 |
| GStreamer | `gstreamer` 0.21 + `gstreamer-app` | gst1-java-core |
| GUI | `eframe` (egui) 0.27+ | Swing |
| 系统托盘 | `tray-icon` 0.14+ | dorkbox SystemTray |
| 全局快捷键 | `global-hotkey` 0.5+ | 无 |
| 开机自启 | `auto-launch` 0.5+ | 无 |
| 配置 | `serde` + `toml` | Spring application.properties |
| 日志 | `tracing` + `tracing-subscriber` | slf4j + logback |
| 错误处理 | `anyhow` + `thiserror` | Java exceptions |
| CLI | `clap` 4.x | 无 |
| 系统通知 | `notify-rust` 4.x | 无 |

### 2.2 选型理由（关键决策）

**为什么是 Rust 而不是 Go / Kotlin**
- 协议层是大量位运算 + 字节操作，Rust 类型系统（`u8` / `[u8; N]` / 模式匹配）远优于 Go / Java
- GStreamer-rs 是 GStreamer 官方维护的绑定，长期跟主线升级
- 单二进制部署，符合"双击即用"目标
- 内存安全，避免 C/C++ 的内存问题

**为什么 GUI 选 egui 而不是 Tauri / Slint**
- 纯 Rust 零外部依赖（Tauri 依赖系统 WebView）
- 即时模式调试方便
- 跟 GStreamer 视频帧渲染无缝（直接更新 `TextureHandle`）
- 启动毫秒级

### 2.3 风险与已接受

| 风险 | 接受理由 |
|------|---------|
| Rust 学习曲线 | 项目周期允许，长期收益大 |
| GStreamer-rs 文档相对少 | 可参考 C 文档 + 上游 Java 实现 |
| FairPlay 翻译易错 | Phase 0 专门做对比测试兜底 |

---

## 3. 项目工程结构

### 3.1 Cargo Workspace 布局

```
airplay-rs/
├── Cargo.toml                  # workspace 根
├── Cargo.lock
├── README.md
├── LICENSE                     # MIT，注明上游 serezhka/java-airplay 来源
├── rust-toolchain.toml         # 固定 Rust 版本
├── .github/workflows/
│   ├── ci.yml                  # 多平台构建 + 测试
│   └── release.yml             # tag 触发多平台产物
│
├── crates/
│   ├── fairplay/               # FairPlay 核心，1:1 翻译
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── omg_hax.rs      # ← OmgHax.java
│   │   │   ├── hand_garble.rs  # ← HandGarble.java
│   │   │   ├── modified_md5.rs # ← ModifiedMD5.java
│   │   │   ├── sap_hash.rs     # ← SapHash.java
│   │   │   └── consts.rs       # ← OmgHaxConst + 10 张表
│   │   ├── tables/             # table_s1..s10（原样拷贝）
│   │   │   ├── table_s1
│   │   │   └── ...
│   │   └── tests/
│   │       └── parity_test.rs  # 与 Java 版输出对比
│   │
│   ├── airplay-protocol/       # 协议层
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── pairing.rs          # ← Pairing.java
│   │   │   ├── fairplay_setup.rs   # ← FairPlay.java
│   │   │   ├── rtsp.rs             # ← RTSP.java
│   │   │   ├── audio_stream_info.rs # ← AudioStreamInfo.java
│   │   │   ├── video_decryptor.rs  # ← FairPlayVideoDecryptor.java
│   │   │   ├── audio_decryptor.rs  # ← FairPlayAudioDecryptor.java
│   │   │   └── plist_util.rs       # ← PropertyListUtil.java
│   │   └── tests/
│   │       └── fixtures/           # 复用上游 one_mirroring_app/
│   │
│   ├── airplay-server/         # 网络层（tokio 重写）
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── server.rs           # ← AirPlayServer.java
│   │   │   ├── bonjour.rs          # ← AirPlayBonjour.java
│   │   │   ├── config.rs           # ← AirPlayConfig.java
│   │   │   ├── consumer.rs         # ← AirPlayConsumer.java（trait）
│   │   │   ├── session.rs          # ← Session.java
│   │   │   ├── control/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── server.rs       # ← ControlServer.java
│   │   │   │   ├── handler.rs      # ← ControlHandler.java
│   │   │   │   └── codec.rs        # ← RtspDecoder/Encoder
│   │   │   ├── video/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── server.rs       # ← VideoServer.java
│   │   │   │   ├── handler.rs      # ← VideoHandler.java
│   │   │   │   └── decoder.rs      # ← VideoDecoder.java
│   │   │   └── audio/
│   │   │       ├── mod.rs
│   │   │       ├── server.rs       # ← AudioServer.java
│   │   │       ├── server_ctrl.rs  # ← AudioControlServer.java
│   │   │       └── handler.rs      # ← AudioHandler.java
│   │   └── tests/
│   │       └── integration_test.rs
│   │
│   ├── airplay-player/         # 播放器实现
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── trait.rs            # Player trait
│   │   │   ├── gstreamer.rs        # ← GstPlayer.java
│   │   │   ├── ffmpeg.rs           # ← FFmpegPlayer.java
│   │   │   └── h264_dump.rs        # ← H264DumpPlayer.java
│   │   └── tests/
│   │
│   └── airplay-cli/            # 可执行入口（GUI + 托盘）
│       ├── Cargo.toml
│       ├── src/
│       │   ├── main.rs
│       │   ├── app.rs              # egui App
│       │   ├── tray.rs             # 托盘逻辑
│       │   ├── settings_window.rs  # 设置面板
│       │   ├── video_window.rs     # 投屏视频窗口
│       │   └── config.rs           # 配置加载/保存
│       ├── assets/
│       │   ├── icon.png            # 托盘图标
│       │   └── icon.ico            # Windows 图标
│       └── release.toml            # 打包配置
│
└── tests/
    └── e2e/                        # 端到端测试（真机投屏）
        └── README.md
```

### 3.2 模块依赖关系

```
                    airplay-cli
                         │
              ┌──────────┼──────────┐
              ▼          ▼          ▼
        airplay-server  airplay-player
              │              │
              └──────┬───────┘
                     ▼
            airplay-protocol
                     │
                     ▼
                 fairplay
```

- `fairplay`：零内部依赖，纯算法 crate
- `airplay-protocol`：依赖 `fairplay` + `plist`/`aes`/`x25519` 等
- `airplay-server`：依赖 `airplay-protocol` + `tokio`/`mdns-sd`
- `airplay-player`：依赖 `airplay-protocol`（仅用 trait）+ `gstreamer`
- `airplay-cli`：装配所有 crate

### 3.3 Cargo.toml 模板

**根 Cargo.toml**

```toml
[workspace]
resolver = "2"
members = [
    "crates/fairplay",
    "crates/airplay-protocol",
    "crates/airplay-server",
    "crates/airplay-player",
    "crates/airplay-cli",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
authors = ["你的名字 <email>"]
repository = "https://github.com/你的用户名/airplay-rs"

[workspace.dependencies]
# 异步
tokio = { version = "1", features = ["full"] }
bytes = "1"
# 加密
aes = "0.8"
ctr = "0.9"
cbc = "0.1"
ed25519-dalek = { version = "2", features = ["rand_core"] }
x25519-dalek = { version = "2", features = ["static_secrets"] }
sha2 = "0.10"
# 序列化
plist = "1"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
# 日志
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
thiserror = "1"
# 网络
mdns-sd = "0.10"
# GUI
eframe = "0.27"
tray-icon = "0.14"
# 测试
pretty_assertions = "1"
```

**`crates/fairplay/Cargo.toml`**

```toml
[package]
name = "fairplay"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
thiserror.workspace = true

[dev-dependencies]
pretty_assertions.workspace = true
hex = "0.4"
```

---

## 4. 开发阶段路线图

### 4.1 总览

| 阶段 | 目标 | 关键产出 | 验证标准 |
|------|------|---------|---------|
| Phase 0 | FairPlay 算法翻译 | `fairplay` crate | 与 Java 版输出字节一致 |
| Phase 1 | 协议层完整实现 | `airplay-protocol` crate | 单元测试覆盖 17 步抓包 |
| Phase 2 | 网络层 + h264-dump | `airplay-server` + 裸流输出 | iPhone 真机能投出 dump.h264 |
| Phase 3 | GStreamer 播放器 | `airplay-player` | iPhone 投屏能看到画面、听到声音 |
| Phase 4 | GUI + 系统托盘 | `airplay-cli` | 双击运行，托盘控制 |
| Phase 5 | 打包发布 | 多平台产物 | 三平台单文件可运行 |

### 4.2 阶段依赖图

```
Phase 0 ─► Phase 1 ─► Phase 2 ─► Phase 3 ─► Phase 4 ─► Phase 5
           (协议)     (网络)      (播放)     (GUI)      (发布)
                          │
                          └─► 中间里程碑：iPhone 真机能 dump.h264
```

### 4.3 时间预估（仅供参考，不作为承诺）

| 阶段 | 预估工作量 | 备注 |
|------|----------|------|
| Phase 0 | 1-2 周 | 含 Rust 学习曲线 |
| Phase 1 | 2-3 周 | 协议层最有把握 |
| Phase 2 | 2-3 周 | tokio 学习 + 调试 |
| Phase 3 | 2-4 周 | GStreamer-rs 调试较耗时 |
| Phase 4 | 2-3 周 | egui 相对简单 |
| Phase 5 | 1-2 周 | CI/CD 配置 |

---

## 5. Phase 0 — FairPlay 翻译可行性验证

> **目标**：验证 Rust 翻译的 FairPlay 算法能产出与 Java 版**字节一致**的解密 key。
> **失败则整个方案放弃**。

### 5.1 任务清单

#### 5.1.1 工程初始化

- [ ] 在 `c:\Users\Administrator\Desktop\` 下创建 `airplay-rs/` 目录
- [ ] `cargo new --bin airplay-rs` 后改造为 workspace
- [ ] 创建 `rust-toolchain.toml` 固定版本：

```toml
[toolchain]
channel = "1.75.0"
components = ["rustfmt", "clippy"]
profile = "default"
```

- [ ] 创建 `crates/fairplay/` 子 crate
- [ ] 拷贝上游 `lib/src/main/resources/table_s1..table_s10` 到 `crates/fairplay/tables/`

#### 5.1.2 翻译 `OmgHaxConst`

- [ ] 把 `OmgHaxConst.java` 中的 `REPLY_MESSAGES`（4 条）和 `KEY_MSG_IDX` 等常量翻译到 `consts.rs`
- [ ] 把 10 张表的加载逻辑用 `include_bytes!` 嵌入二进制：

```rust
// consts.rs
pub const TABLE_S1: &[u8] = include_bytes!("../tables/table_s1");
pub const TABLE_S2: &[u8] = include_bytes!("../tables/table_s2");
// ...
pub const TABLE_S10: &[u8] = include_bytes!("../tables/table_s10");

pub const TABLES: [&[u8]; 10] = [
    TABLE_S1, TABLE_S2, TABLE_S3, TABLE_S4, TABLE_S5,
    TABLE_S6, TABLE_S7, TABLE_S8, TABLE_S9, TABLE_S10,
];

// ← OmgHaxConst.java 中的 REPLY_MESSAGES
pub const REPLY_MESSAGES: [&[u8]; 4] = [
    &[0x46, 0x50, 0x4c, 0x59, /* ... 完整字节 */],
    // ...
];
```

#### 5.1.3 翻译 `OmgHax`

参考 `c:\Users\Administrator\Desktop\airplay\lib\src\main\java\com\github\serezhka\airplay\lib\internal\OmgHax.java`

**关键翻译规则**

| Java 写法 | Rust 写法 | 说明 |
|----------|----------|------|
| `byte b = -106;` | `let b: u8 = 0x96;` | Java byte 有符号，Rust 用 u8 |
| `b & 0xff` | `b`（直接） | u8 不需要掩码 |
| `(b << count) & 0xff` | `b.wrapping_shl(count)` | 注意溢出 |
| `(b & 0xff) >> (8 - count)` | `b.rotate_left(count)` | 整体等价 |
| `byte[] arr = new byte[210];` | `let mut arr = [0u8; 210];` | 固定长度用数组 |
| `arr[i % 210]` | `arr[i % 210]` | usize 自动 |
| `(int) (((i - 155) & 0xffffffffL) % 210)` | `((i.wrapping_sub(155)) % 210)` | i 用 u32 |
| `Math.abs(Math.sin(i + 1))` | `(i as f64 + 1.0).sin().abs()` | 注意 f64 精度 |

**重要约束**

- **必须保留原作者的逆向注释**，例如 `// I have no idea what this is doing (yet)`
- 在每个函数上方加 `// 原 Java: lib/.../OmgHax.java#Lxx-Lyy` 行号引用
- 不要"优化"算法，即使看起来很笨
- 用 `#[allow(clippy::all)]` 在该模块上关掉 lint，避免干扰

#### 5.1.4 翻译 `ModifiedMD5`

参考 `c:\Users\Administrator\Desktop\airplay\lib\src\main\java\com\github\serezhka\airplay\lib\internal\ModifiedMD5.java`

- [ ] `modified_md5` 函数翻译
- [ ] `F` / `G` / `H` / `I` 辅助函数翻译
- [ ] `rol` 用 `u32::rotate_left`
- [ ] `swap` 函数翻译（注意 `ByteBuffer.order(LITTLE_ENDIAN)` 对应 Rust 的 `i32::from_le_bytes`）

#### 5.1.5 翻译 `SapHash`

参考 `c:\Users\Administrator\Desktop\airplay\lib\src\main\java\com\github\serezhka\airplay\lib\internal\SapHash.java`

- [ ] `sap_hash` 主函数
- [ ] `rol8` 用 `u8::rotate_left`
- [ ] 内部调用的 `handGarble.garble(...)` 调用 `HandGarble::garble(...)`

#### 5.1.6 翻译 `HandGarble`

参考 `c:\Users\Administrator\Desktop\airplay\lib\src\main\java\com\github\serezhka\airplay\lib\internal\HandGarble.java`

- [ ] `garble` 主函数
- [ ] `rol8` / `weird_rol8` / `weird_ror8` 函数（注意 `weird_*` 不等于标准 rotate，必须忠实翻译）

#### 5.1.7 编写对比测试

- [ ] 在 Java 项目里加一个 `main` 方法，导出 FairPlay 各阶段的中间状态：

```
pair_setup_step1_request.bin  → 输出
pair_setup_step1_response.bin → 输出
fp_setup_step1_request.bin    → 输出
fp_setup_step2_request.bin    → 输出
最终 videoKey  → hex 字符串
最终 videoIV   → hex 字符串
最终 audioKey  → hex 字符串
```

- [ ] 把这些 hex 字符串作为 `crates/fairplay/tests/parity_test.rs` 的 fixture
- [ ] Rust 侧跑相同输入，断言输出一致：

```rust
#[test]
fn fp_setup_step1_matches_java() {
    let input = hex::decode(include_str!("fixtures/fp_step1_input.hex").trim()).unwrap();
    let expected = hex::decode(include_str!("fixtures/fp_step1_output.hex").trim()).unwrap();
    let actual = fairplay::omg_hax::decrypt_aes_key(&input);
    pretty_assertions::assert_eq!(actual, expected);
}
```

### 5.2 退出条件（Phase 0 完成判据）

- ✅ `cargo test -p fairplay` 全部通过
- ✅ 至少 3 个 fixture（fp-setup step1/step2 + video key 派生）与 Java 版字节一致
- ✅ `cargo clippy` 无 warning（FairPlay 模块可 `#[allow]`）

### 5.3 失败兜底

如果 Phase 0 翻译后输出对不上：

1. **第一步**：检查所有 `byte` → `u8` 转换是否丢失了负号语义
2. **第二步**：用 Java 调试器逐行打印中间状态，Rust 同位置打印，二分定位
3. **第三步**：检查 `Math.sin` vs Rust `sin` 的精度差异（ModifiedMD5 用了 `Math.abs(Math.sin(i+1)) * 2^32`）
4. **第四步**：如果两周仍无法对齐，回头讨论 Go 或 Kotlin 方案

---

## 6. Phase 1 — 协议层完整实现

> **目标**：`airplay-protocol` crate 完整实现 Pairing / FairPlay / RTSP / 解密器，单元测试覆盖 `one_mirroring_app/` 17 步抓包。

### 6.1 任务清单

#### 6.1.1 `Pairing`（pair-verify 两阶段）

参考 `c:\Users\Administrator\Desktop\airplay\lib\src\main\java\com\github\serezhka\airplay\lib\internal\Pairing.java`

- [ ] 翻译 `pairVerify` 二阶段流程
- [ ] 用 `x25519-dalek` 替代 `curve25519-java`
- [ ] 用 `ed25519-dalek` 替代 `net.i2p.crypto.eddsa`
- [ ] AES-CTR 用 `aes` + `ctr` crate
- [ ] 公钥/签名验证逻辑

```rust
// pairing.rs 接口示意
pub struct Pairing {
    server_ed25519_secret: ed25519_dalek::SigningKey,
    shared_secret: Option<[u8; 32]>,
    // ...
}

impl Pairing {
    pub fn new() -> Self { /* 生成 Ed25519 key */ }
    pub fn pair_verify_step1(&mut self, request: &[u8]) -> Result<Vec<u8>>;
    pub fn pair_verify_step2(&mut self, request: &[u8]) -> Result<()>;
    pub fn shared_secret(&self) -> Option<&[u8; 32]>;
}
```

#### 6.1.2 `FairPlaySetup`（/fp-setup）

参考 `c:\Users\Administrator\Desktop\airplay\lib\src\main\java\com\github\serezhka\airplay\lib\internal\FairPlay.java`

- [ ] 4 个 hardcoded 回复消息（来自 `fairplay::consts::REPLY_MESSAGES`）
- [ ] `keyMsg` 存储与 `OmgHax::decryptAesKey` 调用
- [ ] `air_play_stream_key` / `air_play_stream_iv` 派生（FairPlayVideoDecryptor）

#### 6.1.3 `RTSP`（plist 解析）

参考 `c:\Users\Administrator\Desktop\airplay\lib\src\main\java\com\github\serezhka\airplay\lib\internal\RTSP.java`

- [ ] 用 `plist` crate 替代 dd-plist
- [ ] 解析 SETUP / TEARDOWN 请求
- [ ] 提取 `ekey` / `eiv` / `streams`（type 110=video，96=audio）

#### 6.1.4 `FairPlayVideoDecryptor`

参考 `c:\Users\Administrator\Desktop\airplay\lib\src\main\java\com\github\serezhka\airplay\lib\internal\FairPlayVideoDecryptor.java`

- [ ] SHA-512 派生 key：`"AirPlayStreamKey" || streamConnectionID`
- [ ] AES-CTR 解密（key 取 SHA-512 前 16 字节，IV 取后 16 字节）

```rust
pub struct VideoDecryptor {
    cipher: ctr::Ctr128BE<aes::Aes128>,
}

impl VideoDecryptor {
    pub fn new(stream_connection_id: u64, shared_secret: &[u8]) -> Self {
        let mut hasher = sha2::Sha512::new();
        hasher.update(b"AirPlayStreamKey");
        hasher.update(&shared_secret);
        hasher.update(&stream_connection_id.to_be_bytes());
        let result = hasher.finalize();
        let key = &result[..16];
        let iv = &result[16..32];
        // ...
    }
    pub fn decrypt(&mut self, data: &mut [u8]) { /* AES-CTR */ }
}
```

#### 6.1.5 `FairPlayAudioDecryptor`

参考 `c:\Users\Administrator\Desktop\airplay\lib\src\main\java\com\github\serezhka\airplay\lib\internal\FairPlayAudioDecryptor.java`

- [ ] AES-CBC，每次调用重新初始化 IV

#### 6.1.6 `AudioStreamInfo`

参考 `c:\Users\Administrator\Desktop\airplay\lib\src\main\java\com\github\serezhka\airplay\lib\AudioStreamInfo.java`

- [ ] `CompressionType` 枚举（LPCM/ALAC/AAC/AAC_ELD/OPUS）
- [ ] `AudioFormat` 解析

#### 6.1.7 `PropertyListUtil`

参考 `c:\Users\Administrator\Desktop\airplay\server\src\main\java\com\github\serezhka\airplay\server\internal\handler\util\PropertyListUtil.java`

- [ ] 构造 `/info`、`/server-info`、`/playback-info`、SETUP 响应、`/event` 等 plist
- [ ] 用 `plist::Value` 替代 `NSDictionary`

### 6.2 单元测试

为 `one_mirroring_app/` 17 步抓包中每一步编写测试：

```rust
// tests/pairing_test.rs
#[test]
fn step_02_pair_verify_request_matches() {
    let request_bytes = include_bytes!(
        "../fixtures/one_mirroring_app/02_RTSP_POST_pair_verify_request.bin"
    );
    let expected_response = include_bytes!(
        "../fixtures/one_mirroring_app/02_RTSP_POST_pair_verify_response.bin"
    );
    let mut pairing = Pairing::new();
    let actual_response = pairing.pair_verify_step1(request_bytes).unwrap();
    // 比对关键字段（sharedSecret 是随机的，比对结构而非字节）
    assert_pList_structure_match(&actual_response, expected_response);
}
```

### 6.3 退出条件

- ✅ 17 步抓包每一步都有对应单元测试
- ✅ Pairing / FairPlay / RTSP / 解密器全部可独立调用
- ✅ `cargo test -p airplay-protocol` 通过

---

## 7. Phase 2 — 网络层（tokio 重写）

> **目标**：用 tokio 重写 ControlServer / VideoServer / AudioServer，先 dump 到 `dump.h264` 验证可投屏。

### 7.1 任务清单

#### 7.1.1 `AirPlayServer` 入口

参考 `c:\Users\Administrator\Desktop\airplay\server\src\main\java\com\github\serezhka\airplay\server\AirPlayServer.java`

```rust
// server.rs
pub struct AirPlayServer {
    config: Config,
    consumer: Arc<dyn AirPlayConsumer>,
    shutdown: tokio::sync::watch::Receiver<bool>,
}

impl AirPlayServer {
    pub async fn start(self) -> Result<()> {
        let bonjour = tokio::spawn(bonjour::advertise(self.config.clone()));
        let control = tokio::spawn(control::serve(self.config.port, ...));
        // 等待 shutdown 信号
        tokio::select! {
            _ = self.shutdown.changed() => {},
            _ = bonjour => {},
            _ = control => {},
        }
        Ok(())
    }
}
```

#### 7.1.2 `AirPlayBonjour`（mDNS 广播）

参考 `c:\Users\Administrator\Desktop\airplay\lib\src\main\java\com\github\serezhka\airplay\lib\AirPlayBonjour.java`

- [ ] 用 `mdns-sd` 注册 `_airplay._tcp` 和 `_raop._tcp`
- [ ] TXT 记录字段：`deviceid` / `model` / `pk` / `pi` / `gcgl` / `flags` / `vv` 等
- [ ] 假装 AppleTV3,2C

#### 7.1.3 RTSP 编解码器

参考 `c:\Users\Administrator\Desktop\airplay\server\src\main\java\com\github\serezhka\airplay\server\internal\decoder\*.java` 与 `RtspDecoder/Encoder`

- [ ] 用 `tokio_util::codec::Decoder/Encoder` 实现 RTSP 帧解析
- [ ] HTTP 部分用 `httparse` crate（不引入 hyper，控制通道是 RTSP+HTTP 混杂）

#### 7.1.4 `ControlHandler`（核心路由）

参考 `c:\Users\Administrator\Desktop\airplay\server\src\main\java\com\github\serezhka\airplay\server\internal\handler\control\ControlHandler.java`（约 490 行）

按 Java 的方法逐个翻译：

| Java 方法 | Rust 函数 | 说明 |
|----------|----------|------|
| `handleInfo` | `handle_info` | GET /info |
| `handleServerInfo` | `handle_server_info` | GET /server-info |
| `handlePlaybackInfo` | `handle_playback_info` | GET /playback-info |
| `handlePairVerify` | `handle_pair_verify` | POST /pair-verify |
| `handleFpSetup` | `handle_fp_setup` | POST /fp-setup |
| `handleSetup` | `handle_setup` | RTSP SETUP |
| `handleTeardown` | `handle_teardown` | RTSP TEARDOWN |
| `handleGetParameter` | `handle_get_parameter` | RTSP GET_PARAMETER |
| `handleSetParameter` | `handle_set_parameter` | RTSP SET_PARAMETER |
| `handleRecord` | `handle_record` | RTSP RECORD |
| `handleFlush` | `handle_flush` | RTSP FLUSH |
| ... | ... | ... |

**用 enum 替代字符串路由**

```rust
enum ControlRequest {
    GetInfo,
    GetServerInfo,
    PostPairVerify { body: Bytes },
    PostFpSetup { body: Bytes },
    RtspSetup { /* ... */ },
    RtspTeardown { session_id: String },
    // ...
}

async fn handle(req: ControlRequest, state: &SessionState) -> Result<ControlResponse> {
    match req {
        ControlRequest::GetInfo => handle_info(state).await,
        // ...
    }
}
```

#### 7.1.5 `Session` 状态管理

参考 `c:\Users\Administrator\Desktop\airplay\server\src\main\java\com\github\serezhka\airplay\server\internal\handler\session\Session.java`

- [ ] `Arc<Mutex<HashMap<SessionId, Session>>>`
- [ ] 每个 Session 持有：AirPlay 实例、VideoServer、AudioServer、reverseContexts

#### 7.1.6 `VideoServer` + `VideoHandler`

参考 `c:\Users\Administrator\Desktop\airplay\server\src\main\java\com\github\serezhka\airplay\server\internal\handler\video\*.java`

- [ ] TCP 监听（端口由 SETUP 协商）
- [ ] 128 字节 header + payload 解析（ReplayingDecoder 状态机 → 自定义 `tokio_util::codec::Decoder`）
- [ ] 调用 `FairPlayVideoDecryptor` 解密
- [ ] NAL 单元重写：4 字节长度前缀 → `00 00 00 01` Annex-B 起始码
- [ ] SPS/PPS 提取

#### 7.1.7 `AudioServer` + `AudioHandler`

参考 `c:\Users\Administrator\Desktop\airplay\server\src\main\java\com\github\serezhka\airplay\server\internal\handler\audio\*.java`

- [ ] UDP 监听
- [ ] 512 缓冲环按 sequence 重排
- [ ] 调用 `FairPlayAudioDecryptor`

#### 7.1.8 `AirPlayConsumer` trait

参考 `c:\Users\Administrator\Desktop\airplay\server\src\main\java\com\github\serezhka\airplay\server\AirPlayConsumer.java`

```rust
#[async_trait::async_trait]
pub trait AirPlayConsumer: Send + Sync {
    async fn on_video_data(&self, nal_units: Vec<Bytes>) -> Result<()>;
    async fn on_audio_data(&self, packets: Vec<Bytes>, info: AudioStreamInfo) -> Result<()>;
    async fn on_hls_url(&self, url: String) -> Result<()>;
}
```

### 7.2 最简验证：H264DumpConsumer

实现一个最简 `AirPlayConsumer`，把视频 NAL 单元裸写到 `dump.h264`：

```rust
struct H264DumpConsumer(std::sync::Mutex<std::fs::File>);

#[async_trait]
impl AirPlayConsumer for H264DumpConsumer {
    async fn on_video_data(&self, nal_units: Vec<Bytes>) -> Result<()> {
        let mut file = self.0.lock().unwrap();
        for nal in nal_units {
            file.write_all(&nal)?;
        }
        Ok(())
    }
    // ...
}
```

写一个 `examples/dump.rs` 启动 server + H264DumpConsumer。

### 7.3 退出条件

- ✅ iPhone 真机能连上服务（控制中心出现设备名）
- ✅ 点击"屏幕镜像"后 `dump.h264` 持续增长
- ✅ `ffplay dump.h264` 能播放出画面
- ✅ 投屏断开时 TEARDOWN 正确清理资源

### 7.4 这一步的里程碑意义

> **从这一刻起，你已经实现了"能投屏"的最小闭环**。后续只是把 dump 文件换成实时播放。

---

## 8. Phase 3 — GStreamer 播放器集成

> **目标**：用 GStreamer-rs 实时播放视频与音频。

### 8.1 任务清单

#### 8.1.1 GStreamer 环境准备

- [ ] Windows：安装 GStreamer 1.22+ MSVC x86_64，设置 `GSTREAMER_1_0_ROOT_MSVC_X86_64`
- [ ] macOS：`brew install gstreamer`
- [ ] Linux：`apt install gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav`

#### 8.1.2 `GstPlayer` 实现

参考 `c:\Users\Administrator\Desktop\airplay\player\gstreamer\src\main\java\com\github\serezhka\airplay\player\gstreamer\GstPlayer.java`

**三条 pipeline**

1. **视频 pipeline**（H264 解码 + 渲染）：
   ```
   appsrc (NAL) → h264parse → avdec_h264 → videoconvert → glimagesink
   ```
   硬件加速：`avdec_h264` 可换 `vaapih264dec` / `vtdec` / `nvh264dec`

2. **ALAC 音频 pipeline**：
   ```
   appsrc (ALAC frame) → avdec_alac → audioconvert → autoaudiosink
   ```

3. **AAC-ELD 音频 pipeline**：
   ```
   appsrc (AAC-ELD frame) → avdec_aac → audioconvert → autoaudiosink
   ```

4. **HLS pipeline**（YouTube 投屏）：
   ```
   playbin3 uri=... 
   ```

```rust
// gstreamer.rs
pub struct GstPlayer {
    video_pipeline: gst::Pipeline,
    video_appsrc: gst_app::AppSrc,
    audio_pipeline: Option<gst::Pipeline>,
    audio_appsrc: Option<gst_app::AppSrc>,
}

impl GstPlayer {
    pub fn new(video_sink: Box<dyn VideoSink>) -> Result<Self> { /* 构造 pipeline */ }
    pub fn push_video(&self, nal: Bytes) -> Result<()> { /* appsrc.push_buffer */ }
    pub fn push_audio(&self, frame: Bytes) -> Result<()> { /* appsrc.push_buffer */ }
}
```

#### 8.1.3 视频渲染到 egui 纹理

GStreamer 的 `glimagesink` 默认创建自己的窗口。要在 egui 内嵌渲染：

- 方案 A（推荐 MVP）：用 `appsink` 取出原始 RGBA 帧，推到 egui `TextureHandle`
- 方案 B（性能更好）：用 GStreamer 的 `glsinkbin` 或共享 GL context（复杂）

```rust
// 视频帧 → egui 纹理
fn on_new_frame(&mut self, frame: &[u8], width: u32, height: u32, ctx: &egui::Context) {
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [width as usize, height as usize],
        frame,
    );
    let texture = ctx.load_texture("video_frame", image, egui::TextureOptions::LINEAR);
    self.video_texture = Some(texture);
}
```

#### 8.1.4 实现 `AirPlayConsumer` for `GstPlayer`

```rust
#[async_trait]
impl AirPlayConsumer for GstPlayer {
    async fn on_video_data(&self, nal_units: Vec<Bytes>) -> Result<()> {
        for nal in nal_units {
            self.push_video(nal)?;
        }
        Ok(())
    }
    async fn on_audio_data(&self, packets: Vec<Bytes>, info: AudioStreamInfo) -> Result<()> {
        match info.compression {
            CompressionType::Alac => { /* push to ALAC pipeline */ }
            CompressionType::AacEld => { /* push to AAC-ELD pipeline */ }
            _ => warn!("unsupported audio: {:?}", info.compression),
        }
        Ok(())
    }
}
```

### 8.2 退出条件

- ✅ iPhone 投屏能看到实时画面，延迟 < 500ms
- ✅ 能听到音频（无爆音、无断续）
- ✅ 投屏停止时 pipeline 干净退出，无残留进程

### 8.3 已知坑

- GStreamer pipeline 状态机调试困难，建议每个 pipeline 加 `gst::debug_set_default_threshold(gst::DebugLevel::Info)` 看日志
- `appsrc` 的 `max-bytes` / `leaky-type` 要设置，否则背压会卡
- AAC-ELD 需要 `gst-plugins-bad` + `libfdk-aac`，Windows 下可能要自己编译

---

## 9. Phase 4 — GUI + 系统托盘

> **目标**：双击单二进制启动，托盘控制，视频窗口自动弹出。

### 9.1 任务清单

#### 9.1.1 主进程架构

```rust
// main.rs
fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    // 加载配置
    let config = Config::load_or_default()?;

    // 启动 tokio runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    // 启动 AirPlay server（后台 task）
    let server = rt.block_on(async {
        AirPlayServer::new(config.airplay.clone()).start()
    });

    // 启动 egui 主线程（阻塞）
    let app = AirPlayApp::new(server, config);
    eframe::run_native(
        "AirPlay Receiver",
        eframe::NativeOptions::default(),
        Box::new(|cc| Box::new(app)),
    )?;

    Ok(())
}
```

#### 9.1.2 系统托盘

参考 `c:\Users\Administrator\Desktop\airplay\player\app\src\main\java\com\github\serezhka\airplay\app\PlayerApp.java`（Java 版用 dorkbox SystemTray）

```rust
// tray.rs
pub fn create_tray(state: Arc<AppState>) -> Result<tray_icon::TrayIcon> {
    let icon = load_icon();
    let menu = Menu::new(&state)?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("AirPlay Receiver")
        .with_icon(icon)
        .build()?;
    Ok(tray)
}
```

托盘菜单项：
- ✓ AirPlay 已启用 / ✗ AirPlay 已禁用（切换）
- 当前设备名
- ─────
- 设置...
- 查看日志
- 关于
- 退出

#### 9.1.3 设置面板

egui 实现配置编辑：

```rust
// settings_window.rs
pub fn show(ctx: &egui::Context, config: &mut Config, open: &mut bool) {
    egui::Window::new("设置")
        .open(open)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("设备名称");
            ui.text_edit_singleline(&mut config.airplay.server_name);

            ui.label("视频分辨率");
            egui::ComboBox::from_label("")
                .selected_text(format!("{}×{}", config.airplay.width, config.airplay.height))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut config.airplay.width, 1280, "1280×720");
                    ui.selectable_value(&mut config.airplay.width, 1920, "1920×1080");
                });

            ui.checkbox(&mut config.auto_start, "开机自启");
            ui.checkbox(&mut config.hardware_decode, "硬件加速解码");

            ui.horizontal(|ui| {
                if ui.button("取消").clicked() { *open = false; }
                if ui.button("保存并重启").clicked() {
                    config.save().ok();
                    *open = false;
                }
            });
        });
}
```

#### 9.1.4 视频窗口

- 投屏开始时自动弹出（`Viewport` API）
- 投屏结束时自动关闭
- 顶栏显示连接设备、码率、延迟
- 可选录制按钮

#### 9.1.5 配置持久化

`config.toml`（替代 `application.properties`）：

```toml
[airplay]
server_name = "MyReceiver"
width = 1920
height = 1080
fps = 60

[player]
implementation = "gstreamer"
hardware_decode = true

[ui]
tray_enabled = true
auto_start = false
minimize_to_tray = true
```

存放位置：
- Windows：`%APPDATA%\airplay-rs\config.toml`
- macOS：`~/Library/Application Support/airplay-rs/config.toml`
- Linux：`~/.config/airplay-rs/config.toml`

用 `directories` crate 自动获取路径。

#### 9.1.6 开机自启

```rust
// 用 auto-launch crate
let auto = auto_launch::AutoLaunchBuilder::new()
    .set_app_name("AirPlayReceiver")
    .set_app_path(std::env::current_exe()?.to_str().unwrap())
    .build()?;
auto.enable()?;
```

#### 9.1.7 全局快捷键

用 `global-hotkey` 注册 `Ctrl+Alt+A` 切换 AirPlay 开关。

### 9.2 退出条件

- ✅ Windows 双击 `airplay-receiver.exe` 启动，托盘出现图标
- ✅ 设置面板可改设备名、分辨率，保存后重启生效
- ✅ iPhone 投屏时视频窗口自动弹出
- ✅ 关闭主窗口后程序在托盘继续运行
- ✅ macOS / Linux 同样行为

---

## 10. Phase 5 — 打包与发布

### 10.1 三平台构建

#### 10.1.1 Windows

```shell
cargo build --release --target x86_64-pc-windows-msvc
```

- 用 `cargo-wix` 生成 MSI 安装包
- 或用 Inno Setup 打包
- 静态链接 GStreamer？不能，但可在 README 注明依赖

#### 10.1.2 macOS

```shell
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
lipo -create -output airplay-receiver target/aarch64-apple-darwin/release/airplay-receiver target/x86_64-apple-darwin/release/airplay-receiver
```

- 用 `cargo-bundle` 生成 `.app`
- 代码签名（Developer ID Application）
- 公证（notarization）

#### 10.1.3 Linux

```shell
cargo build --release --target x86_64-unknown-linux-gnu
```

- 提供 `.deb`（用 `cargo-deb`）
- 提供 `.AppImage`（用 `linuxdeploy` + `appimagetool`）
- 提供 Flatpak（可选）

### 10.2 CI/CD（GitHub Actions）

`.github/workflows/release.yml`：

```yaml
name: Release
on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: airplay-receiver.exe
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact: airplay-receiver
          - os: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
            artifact: airplay-receiver
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Install GStreamer (Windows)
        if: matrix.os == 'windows-latest'
        run: choco install gstreamer-devel
      - name: Install GStreamer (macOS)
        if: matrix.os == 'macos-latest'
        run: brew install gstreamer
      - name: Install GStreamer (Linux)
        if: matrix.os == 'ubuntu-22.04'
        run: sudo apt install -y gstreamer1.0-plugins-{base,good,bad,libav}
      - run: cargo build --release
      - uses: softprops/action-gh-release@v1
        with:
          files: target/release/${{ matrix.artifact }}
```

### 10.3 README 必备内容

- 一句话简介 + 截图
- 下载链接（GitHub Releases）
- GStreamer 安装说明（三平台）
- 使用方法（双击 → iPhone 控制中心选择 → 投屏）
- 故障排查（防火墙、同一 WiFi、GStreamer 路径）
- 致谢上游 `serezhka/java-airplay`
- MIT 许可证

### 10.4 退出条件

- ✅ 三平台 `tag → CI → Release` 自动出包
- ✅ 在干净系统上能跑通（仅装 GStreamer）
- ✅ README 完整

---

## 11. 关键技术细节

### 11.1 FairPlay 翻译检查清单

每翻译一个 FairPlay 函数，对照以下检查：

- [ ] 所有 `byte` 已转为 `u8`
- [ ] 所有 `int` / `long` 已用 `u32` / `u64` 或 `usize`
- [ ] `& 0xff` / `& 0xffffffffL` 已正确处理（u8/u32 类型本身不需要）
- [ ] `<< count` 改为 `wrapping_shl` 或 `rotate_left`
- [ ] 数组索引的负数取模已用 `wrapping_sub` + `u32`
- [ ] `Math.abs(Math.sin(i+1))` 已用 `(i+1) as f64` 的 sin
- [ ] `ByteBuffer.order(LITTLE_ENDIAN)` 已用 `from_le_bytes`
- [ ] 数组长度与 Java 一致
- [ ] 注释保留原作者逆向说明
- [ ] 加了 `// 原 Java: xxx.java#Lxx` 行号引用

### 11.2 RTSP 解析注意点

原 Java 用 Netty 自带的 `HttpResponseDecoder` 兼容 RTSP。Rust 没有现成 RTSP 解析器（`rtsp` crate 较旧），需要自己写：

```rust
// 自定义 codec
#[derive(Debug)]
enum RtspMessage {
    Request { method: String, uri: String, headers: Vec<(String, String)>, body: Bytes },
    Response { status: u16, headers: Vec<(String, String)>, body: Bytes },
}

impl Decoder for RtspCodec {
    type Item = RtspMessage;
    type Error = anyhow::Error;
    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<RtspMessage>> {
        // 1. 找 \r\n\r\n 分隔 header/body
        // 2. 解析请求行 / 状态行
        // 3. 解析 headers
        // 4. 读 Content-Length 字节作为 body
    }
}
```

### 11.3 音频包重排序

参考 `c:\Users\Administrator\Desktop\airplay\server\src\main\java\com\github\serezhka\airplay\server\internal\handler\audio\AudioHandler.java`（512 缓冲环）

```rust
struct AudioReorderBuffer {
    buffer: [Option<Bytes>; 512],
    next_seq: u16,
}

impl AudioReorderBuffer {
    fn push(&mut self, seq: u16, data: Bytes) -> Vec<Bytes> {
        let idx = seq % 512;
        self.buffer[idx] = Some(data);
        let mut out = vec![];
        while let Some(data) = self.buffer[self.next_seq as usize % 512].take() {
            out.push(data);
            self.next_seq = self.next_seq.wrapping_add(1);
        }
        out
    }
}
```

### 11.4 H264 NAL 重写

参考 `c:\Users\Administrator\Desktop\airplay\server\src\main\java\com\github\serezhka\airplay\server\internal\handler\video\VideoHandler.java`

AirPlay 视频流格式：4 字节大端长度前缀 + NAL 数据。GStreamer / ffplay 需要 Annex-B 格式（`00 00 00 01` 起始码）。

```rust
fn rewrite_nal_units(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() + 32);
    let mut offset = 0;
    while offset + 4 <= input.len() {
        let len = u32::from_be_bytes([
            input[offset], input[offset+1], input[offset+2], input[offset+3]
        ]) as usize;
        offset += 4;
        if offset + len > input.len() { break; }
        out.extend_from_slice(&[0, 0, 0, 1]); // start code
        out.extend_from_slice(&input[offset..offset+len]);
        offset += len;
    }
    out
}
```

### 11.5 错误处理策略

```rust
// 协议层：用 thiserror 定义具体错误
#[derive(thiserror::Error, Debug)]
pub enum ProtocolError {
    #[error("invalid plist: {0}")]
    InvalidPlist(String),
    #[error("fairplay decryption failed")]
    FairPlayFailed,
    #[error("pairing step {step} failed: {reason}")]
    PairingFailed { step: u8, reason: String },
}

// 应用层：用 anyhow 传播
type Result<T> = std::result::Result<T, anyhow::Error>;
```

---

## 12. 测试策略

### 12.1 测试金字塔

```
        ┌─────────────────┐
        │  E2E（真机投屏） │  ← Phase 5 之后
        ├─────────────────┤
        │ 集成测试（多 crate）│ ← Phase 2/3
        ├─────────────────┤
        │ 单元测试（单 crate）│ ← Phase 0/1
        └─────────────────┘
```

### 12.2 FairPlay 对比测试（关键）

把 Java 版作为"金标准"：

1. Java 项目里加一个 `main`，输入 `one_mirroring_app/04_RTSP_POST_fp_setup_request.bin`，输出中间状态 hex
2. Rust 测试用相同输入，断言输出一致

```rust
#[test]
fn fp_setup_step1_output_matches_java() {
    let input = include_bytes!("fixtures/04_fp_setup_request.bin");
    let expected = include_str!("fixtures/04_fp_setup_step1_output.hex");
    let actual = hex::encode(fairplay::process_fp_setup_step1(input));
    assert_eq!(actual, expected.trim());
}
```

### 12.3 协议层 17 步抓包测试

每一步都建一个测试函数，输入 `.bin`，验证响应结构（部分字段如 sharedSecret 是随机的不能直接比对）。

### 12.4 网络层集成测试

```rust
#[tokio::test]
async fn control_server_responds_to_info() {
    let server = AirPlayServer::start_test().await;
    let mut stream = TcpStream::connect(server.addr()).await.unwrap();
    stream.write_all(b"GET /info RTSP/1.0\r\n\r\n").await.unwrap();
    let response = read_response(&mut stream).await;
    assert!(response.contains("AirPlay"));
}
```

### 12.5 真机 E2E

手动测试清单：
- [ ] iPhone 控制中心能看到设备
- [ ] 点击屏幕镜像能连上
- [ ] 画面延迟 < 500ms
- [ ] 音频无爆音
- [ ] 投屏停止后能再次连接
- [ ] 长时间投屏（30 分钟）无崩溃

---

## 13. 长期维护策略

### 13.1 跟随 GStreamer 主线

每 6 个月 GStreamer 大版本发布时：
- 升级 `gstreamer-rs` crate
- 跑回归测试
- 评估新硬件解码器（如未来 AV1 硬解）

### 13.2 跟随 Rust 版本

每年 Rust 新 edition 发布时评估升级，但 `Cargo.lock` 锁定具体版本以保证可复现构建。

### 13.3 跟随 AirPlay 协议变化

iOS 大版本发布后用 Wireshark 抓包，对比 `one_mirroring_app/` 看协议变化。已知变化点：
- iOS 18+ 可能要求新的 pair-setup 流程
- AirPlay 3 引入的 timing 协议变化

### 13.4 依赖审计

每月跑 `cargo audit` 检查安全漏洞。

### 13.5 CI 守护

每次 push 跑：
- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test --workspace`
- 三平台跨编译验证

---

## 14. 风险与应对

| 风险 | 概率 | 影响 | 应对 |
|------|------|------|------|
| FairPlay 翻译对不上 | 中 | 致命 | Phase 0 专门验证，失败则切换 Kotlin 方案 |
| GStreamer-rs API 变化 | 低 | 中 | 锁版本 + 升级时回归测试 |
| AAC-ELD 在某些平台不可用 | 中 | 中 | 文档说明 + 提供 ALAC 回退 |
| iOS 升级导致协议变化 | 中 | 高 | 抓包对比 + 协议层独立可改 |
| Rust 学习曲线 | 高 | 中 | Phase 0 给足时间，不催进度 |
| 投屏延迟过高 | 低 | 中 | 启用硬件解码 + 调 GStreamer latency 参数 |

---

## 15. 附录

### 15.1 Java → Rust 文件迁移对照表

| Java 文件 | Rust 文件 | Phase |
|----------|----------|-------|
| `lib/internal/OmgHax.java` | `crates/fairplay/src/omg_hax.rs` | 0 |
| `lib/internal/HandGarble.java` | `crates/fairplay/src/hand_garble.rs` | 0 |
| `lib/internal/ModifiedMD5.java` | `crates/fairplay/src/modified_md5.rs` | 0 |
| `lib/internal/SapHash.java` | `crates/fairplay/src/sap_hash.rs` | 0 |
| `lib/internal/OmgHaxConst.java` | `crates/fairplay/src/consts.rs` | 0 |
| `lib/resources/table_s1..s10` | `crates/fairplay/tables/table_s1..s10` | 0 |
| `lib/internal/Pairing.java` | `crates/airplay-protocol/src/pairing.rs` | 1 |
| `lib/internal/FairPlay.java` | `crates/airplay-protocol/src/fairplay_setup.rs` | 1 |
| `lib/internal/FairPlayVideoDecryptor.java` | `crates/airplay-protocol/src/video_decryptor.rs` | 1 |
| `lib/internal/FairPlayAudioDecryptor.java` | `crates/airplay-protocol/src/audio_decryptor.rs` | 1 |
| `lib/internal/RTSP.java` | `crates/airplay-protocol/src/rtsp.rs` | 1 |
| `lib/AirPlay.java` | `crates/airplay-protocol/src/lib.rs` | 1 |
| `lib/AudioStreamInfo.java` | `crates/airplay-protocol/src/audio_stream_info.rs` | 1 |
| `lib/AirPlayBonjour.java` | `crates/airplay-server/src/bonjour.rs` | 2 |
| `server/AirPlayServer.java` | `crates/airplay-server/src/server.rs` | 2 |
| `server/AirPlayConfig.java` | `crates/airplay-server/src/config.rs` | 2 |
| `server/AirPlayConsumer.java` | `crates/airplay-server/src/consumer.rs` | 2 |
| `server/internal/ControlServer.java` | `crates/airplay-server/src/control/server.rs` | 2 |
| `server/internal/handler/control/ControlHandler.java` | `crates/airplay-server/src/control/handler.rs` | 2 |
| `server/internal/handler/session/Session.java` | `crates/airplay-server/src/session.rs` | 2 |
| `server/internal/handler/video/VideoHandler.java` | `crates/airplay-server/src/video/handler.rs` | 2 |
| `server/internal/handler/video/VideoDecoder.java` | `crates/airplay-server/src/video/decoder.rs` | 2 |
| `server/internal/handler/audio/AudioHandler.java` | `crates/airplay-server/src/audio/handler.rs` | 2 |
| `server/internal/handler/util/PropertyListUtil.java` | `crates/airplay-protocol/src/plist_util.rs` | 1 |
| `player/gstreamer/.../GstPlayer.java` | `crates/airplay-player/src/gstreamer.rs` | 3 |
| `player/ffmpeg/.../FFmpegPlayer.java` | `crates/airplay-player/src/ffmpeg.rs` | 3 |
| `player/h264-dump/.../H264DumpPlayer.java` | `crates/airplay-player/src/h264_dump.rs` | 2 |
| `player/app/.../PlayerApp.java` | `crates/airplay-cli/src/main.rs` | 4 |
| `player/app/.../config/PlayerConfig.java` | `crates/airplay-cli/src/config.rs` | 4 |
| `player/app/resources/application.properties` | `crates/airplay-cli/config.toml.example` | 4 |

### 15.2 不迁移的文件

| Java 文件 | 不迁移原因 |
|----------|----------|
| `client/**` | 不做发送端 |
| `player/vlc/**` | vlcj 实验性，GStreamer 已够用 |
| `server/src/test/java/.../ReverseEngineeringTest.java` | 仅是逆向辅助，不复用 |
| Spring Boot 相关 | 用 Rust 替代 |

### 15.3 重要常量速查

- AirPlay 控制端口：7100（默认）
- RAOP 控制端口：与 AirPlay 同
- 视频流 type：110
- 音频流 type：96
- FairPlay stream key 派生：`SHA-512("AirPlayStreamKey" + sharedSecret + streamConnectionID)`，前 16 字节为 AES key
- FairPlay stream IV 派生：`SHA-512("AirPlayStreamIV" + sharedSecret + streamConnectionID)`，前 16 字节为 IV
- mDNS 服务：`_airplay._tcp` + `_raop._tcp`

### 15.4 学习资料

- **GStreamer-rs 官方教程**：https://gitlab.freedesktop.org/gstreamer/gstreamer-rs
- **egui 官方示例**：https://github.com/emilk/egui
- **tokio 教程**：https://tokio.rs/tokio/tutorial
- **原 Java 项目 README**：`c:\Users\Administrator\Desktop\airplay\README.md`
- **协议抓包参考**：`c:\Users\Administrator\Desktop\airplay\server\src\test\resources\one_mirroring_app\`

### 15.5 致谢

本项目基于 `serezhka/java-airplay` 的 FairPlay 逆向成果（MIT 许可），向原作者 Serezhka 致谢。FairPlay 算法模块（`crates/fairplay/`）保留原作者的逆向注释，仅做语言翻译。

---

## 16. 立即行动项

读完本文档后，可以立即开始的第一步：

1. 在 `c:\Users\Administrator\Desktop\` 下创建 `airplay-rs/` 目录
2. 初始化 Cargo workspace
3. 创建 `crates/fairplay/` 子 crate
4. 拷贝 10 张置换表
5. 翻译 `OmgHaxConst` → `consts.rs`
6. 翻译 `OmgHax` → `omg_hax.rs`
7. 在 Java 项目里写一个 `main` 导出 FairPlay 测试 vector
8. 在 Rust 里写对比测试

完成 Phase 0 后，回到本文档看 Phase 1。

---

**文档版本**：v1.0
**编写日期**：2026-07-13
**最后修订**：2026-07-13
