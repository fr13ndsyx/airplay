# airplay-rs

AirPlay 接收端的 Rust 重构项目。基于上游 Java 项目 `serezhka/java-airplay`（MIT 许可）。

## 当前状态

**Phase 0 — FairPlay 核心翻译与对比验证**（进行中）

- ✅ Cargo workspace 骨架已搭建
- ✅ 10 张置换表已从 Java 项目拷贝
- 🔄 5 个 FairPlay 核心 Java 文件正在翻译为 Rust
- ⏳ Java 测试向量导出工具待编写
- ⏳ Rust 对比测试待编写

详见 [RUST_REWRITE_DEV_PLAN.md](./RUST_REWRITE_DEV_PLAN.md) 了解完整的 6 阶段开发路线图。

## 构建

```shell
cargo build -p fairplay
cargo test -p fairplay
cargo clippy -p fairplay
```

## 致谢

本项目基于 `serezhka/java-airplay` 的 FairPlay 逆向成果（MIT 许可）。`crates/fairplay/` 模块保留原作者的逆向注释，仅做语言翻译。
