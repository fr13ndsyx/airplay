# airplay-rs 项目结构说明

> Rust 重构版 AirPlay 接收器，基于 Java 版 `airplay` 项目翻译而来。

## 目录树

```
airplay-rs/
│
├── .cargo/
│   └── config.toml                 # Cargo 构建配置（GStreamer 库路径、链接器参数等）
│
├── .gitignore                      # Git 忽略规则（target/、*.log 等）
│
├── .trae-cn/                       # Trae IDE 开发过程文件（可删除）
│   └── specs/
│       └── translate-fairplay-remaining/
│           ├── spec.md             # FairPlay 翻译规格说明
│           └── tasks.md            # 翻译任务清单
│
├── Cargo.toml                      # 工作空间根配置，定义 5 个子 crate 及共享依赖
├── Cargo.lock                      # 依赖版本锁定文件
├── rust-toolchain.toml             # Rust 工具链版本指定（nightly）
├── README.md                       # 项目说明文档
├── RUST_REWRITE_DEV_PLAN.md        # Rust 重构开发计划文档
├── encrypted_payload               # FairPlay 测试用的加密负载数据
├── play_stderr.log                 # 播放器标准错误日志
├── play_stdout.log                 # 播放器标准输出日志
├── stderr.log                      # 程序标准错误日志
├── stdout.log                      # 程序标准输出日志
│
└── crates/                         # ─── 所有 Rust 源代码 ───
    │
    ├── airplay-cli/                # [可执行 crate] 桌面应用入口
    │   ├── Cargo.toml
    │   ├── src/
    │   │   ├── main.rs             # 程序入口，启动 eframe GUI 事件循环
    │   │   ├── lib.rs              # crate 根模块导出
    │   │   ├── app.rs              # egui GUI 界面（启停按钮、音量滑块、状态显示）
    │   │   ├── server_task.rs      # 后台 server 任务管理（Start/Stop/SetVolume 命令分发）
    │   │   ├── status.rs           # 状态枚举（Idle/Running/Connected/Disconnected）与命令枚举
    │   │   ├── status_consumer.rs  # 状态消费者：将 server 状态变化转发给 GUI 线程
    │   │   └── tray.rs             # 系统托盘图标与右键菜单
    │   └── tests/
    │       └── integration_test.rs # CLI 集成测试
    │
    ├── airplay-player/             # [库 crate] GStreamer 音视频播放器
    │   ├── Cargo.toml
    │   ├── examples/
    │   │   └── play.rs             # 独立播放器示例（不依赖 AirPlay 协议）
    │   ├── src/
    │   │   ├── lib.rs              # crate 根模块导出
    │   │   ├── player.rs           # GstPlayer：管理视频/音频管线，接收视频帧和音频数据，音量控制
    │   │   ├── video_pipeline.rs   # GStreamer 视频管线构建（appsrc → decode → glupload → glimagesink）
    │   │   ├── audio_pipeline.rs   # GStreamer 音频管线构建（ALAC 和 AAC-ELD 两条管线，含 volume 元素）
    │   │   └── consumer.rs         # 消费者 trait：server → player 的视频/音频数据桥接
    │   └── tests/
    │       └── integration_test.rs # 播放器集成测试
    │
    ├── airplay-protocol/           # [库 crate] AirPlay 协议实现
    │   ├── Cargo.toml
    │   ├── src/
    │   │   ├── lib.rs              # crate 根模块导出
    │   │   ├── airplay.rs          # AirPlay 协议主逻辑（info、pair-setup、pair-verify、fp-setup）
    │   │   ├── fairplay_setup.rs   # FairPlay 握手流程（3 轮 fp-setup 交换）
    │   │   ├── pairing.rs          # Ed25519 配对验证（公钥交换、签名验证）
    │   │   ├── rtsp.rs             # RTSP 请求/响应编解码与处理
    │   │   ├── plist_util.rs       # plist（二进制/XML）序列化与反序列化工具
    │   │   ├── stream_info.rs      # 流信息解析（编解码器类型、SPS/PPS 等参数）
    │   │   ├── video_decryptor.rs  # 视频流 AES-CTR 解密器（FairPlay 密钥派生 + 状态计数器）
    │   │   └── audio_decryptor.rs  # 音频流 AES-CTR 解密器
    │   ├── tests/
    │   │   ├── fairplay_setup_test.rs  # FairPlay 握手流程测试
    │   │   ├── fairplay_test.rs        # FairPlay 解密端到端测试
    │   │   ├── pairing_test.rs         # 配对验证测试
    │   │   ├── plist_util_test.rs      # plist 工具测试
    │   │   ├── rtsp_test.rs            # RTSP 编解码测试
    │   │   └── fixtures/
    │   │       └── one_mirroring_app/  # 真机抓包数据（17 组 RTSP/HTTP 请求-响应对）
    │   │           ├── 00_mdns_records.png/txt   # mDNS 服务发现记录
    │   │           ├── 01_RTSP_GET_info_*.bin    # RTSP info 请求/响应
    │   │           ├── 02-03_RTSP_POST_pair_verify_*.bin  # 配对验证（2 轮）
    │   │           ├── 04-05_RTSP_POST_fp_setup_*.bin     # FairPlay 设置（2 轮）
    │   │           ├── 06_RTSP_SETUP_*.bin        # RTSP SETUP（建立流通道）
    │   │           ├── 07_RTSP_GET_PARAMETER_*.bin # 参数查询
    │   │           ├── 08_RTSP_RECORD_*.bin       # 录制开始
    │   │           ├── 09_RTSP_SET_PARAMETER_*.bin # 设置参数（视频格式等）
    │   │           ├── 10_RTSP_SETUP_*.bin        # 第二次 SETUP
    │   │           ├── 11_RTSP_FLUSH_*.bin        # 流刷新
    │   │           ├── 12_HTTP_GET_server_info_*.bin  # HTTP server-info
    │   │           ├── 13_HTTP_POST_fp_setup2_*.bin   # 第二阶段 FairPlay
    │   │           ├── 14_RTSP_POST_audio_mode_*.bin  # 音频模式设置
    │   │           ├── 15_RTSP_POST_feedback_*.bin    # 反馈
    │   │           ├── 16_RTSP_SET_PARAMETER_*.bin    # 参数更新
    │   │           └── 17_RTSP_TEARDOWN_*.bin     # 断开连接
    │
    ├── airplay-server/             # [库 crate] AirPlay 服务端（网络监听与数据路由）
    │   ├── Cargo.toml
    │   ├── examples/
    │   │   ├── dump.rs             # 投屏数据抓取示例（保存 H.264 帧到文件）
    │   │   ├── hello.rs            # 最小可用示例
    │   │   └── smoke.rs            # 冒烟测试示例
    │   ├── src/
    │   │   ├── lib.rs              # crate 根模块导出
    │   │   ├── server.rs           # AirPlayServer：主服务器，协调所有子服务
    │   │   ├── config.rs           # 服务器配置（端口、设备名、硬件信息等）
    │   │   ├── session.rs          # AirPlay 会话管理（每台设备的连接状态）
    │   │   ├── bonjour.rs          # mDNS/Bonjour 服务广播（让 iPhone 发现本设备）
    │   │   ├── rtsp_codec.rs       # RTSP 消息编解码器（tokio codec）
    │   │   ├── consumer.rs         # Consumer trait：接收解密后的视频/音频帧并转发给播放器
    │   │   ├── h264_dump.rs        # H.264 帧转储工具（调试用）
    │   │   ├── audio/              # 音频子模块
    │   │   │   ├── mod.rs          # 模块导出
    │   │   │   ├── server.rs       # 音频流服务器（接收 ALAC/AAC-ELD RTP 包）
    │   │   │   ├── server_ctrl.rs  # 音频控制服务器（接收音量等控制指令）
    │   │   │   ├── rtp.rs          # RTP 包解析
    │   │   │   └── reorder.rs      # RTP 包乱序重排
    │   │   ├── video/              # 视频子模块
    │   │   │   ├── mod.rs          # 模块导出
    │   │   │   ├── server.rs       # 视频流服务器（接收 H.264 RTP 包，FairPlay 解密）
    │   │   │   ├── decoder.rs      # H.264 NALU 解析与 SPS/PPS 提取
    │   │   │   └── nal.rs          # NAL 单元类型判断与处理
    │   │   └── control/            # 控制通道子模块
    │   │       ├── mod.rs          # 模块导出
    │   │       ├── server.rs       # 控制服务器（事件回调、时间同步）
    │   │       └── handler.rs      # 控制消息处理器
    │   └── tests/
    │       └── integration_test.rs # 服务器集成测试
    │
    └── fairplay/                   # [库 crate] FairPlay DRM 解密核心（从 Java OmgHax 翻译）
        ├── Cargo.toml
        ├── src/
        │   ├── lib.rs              # crate 根模块导出
        │   ├── consts.rs           # 常量表（INITIAL_SESSION_KEY、DEFAULT_SAP、MESSAGE_KEY、查找表数据）
        │   ├── omg_hax.rs          # OmgHax 核心算法：AES 密钥解密、会话密钥生成、密钥调度
        │   ├── sap_hash.rs         # SapHash：840 轮混淆哈希算法
        │   ├── hand_garble.rs      # HandGarble：字节扰乱变换
        │   └── modified_md5.rs     # ModifiedMD5：自定义 MD5 变体（含 sin 常数和位运算差异）
        ├── tables/                 # FairPlay 查找表数据文件（二进制/文本格式）
        │   ├── table_s1 ~ table_s4 # 二进制查找表（Long::decode 解析）
        │   ├── table_s5 ~ table_s9 # 文本格式查找表
        │   └── table_s10           # 二进制查找表
        └── tests/
            └── parity_test.rs      # Java/Rust 一致性测试（12 组测试向量，验证翻译正确性）
```

## 架构概览

```
┌──────────────────────────────────────────────────────────┐
│                    airplay-cli (GUI)                      │
│  egui 界面 / 系统托盘 / 启停控制 / 音量调节               │
├──────────────────────────────────────────────────────────┤
│                   airplay-server                          │
│  mDNS 广播 → RTSP 握手 → 音频/视频/控制通道监听           │
├──────────────┬───────────────────┬───────────────────────┤
│ fairplay     │  airplay-protocol │  airplay-player       │
│ DRM 解密核心 │  协议编解码/密钥   │  GStreamer 播放管线   │
└──────────────┴───────────────────┴───────────────────────┘
```

## 数据流

```
iPhone
  │ mDNS 广播发现
  ▼
airplay-server (bonjour.rs)
  │ RTSP 握手 + FairPlay 认证
  ▼
airplay-protocol (fairplay_setup.rs → fairplay/omg_hax.rs)
  │ 密钥协商完成，开始传输加密流
  ▼
airplay-server/video/server.rs ──→ video_decryptor.rs ──→ consumer.rs
  │                                                      │
  ▼                                                      ▼
airplay-server/audio/server.rs ──→ audio_decryptor.rs ──→ consumer.rs
                                                         │
                                                         ▼
                                                   airplay-player
                                                   (GStreamer 渲染)
```

## Crate 依赖关系

```
airplay-cli
  ├── airplay-server
  │   ├── airplay-protocol
  │   │   └── fairplay
  │   └── airplay-player
  └── (egui, tray-icon 等外部依赖)

airplay-server
  ├── airplay-protocol
  └── airplay-player
```
