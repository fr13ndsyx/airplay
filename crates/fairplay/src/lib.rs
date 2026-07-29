//! FairPlay 核心算法 crate
//!
//! FairPlay 逆向算法的 Rust 实现。
//! 算法本身是逆向产物，实现时严格遵循 1:1 对应原则，不做任何"优化"。
//!
//! 模块结构：
//! - [`consts`]：常量定义模块
//! - [`omg_hax`]：主入口模块
//! - [`modified_md5`]：修改版 MD5 模块
//! - [`sap_hash`]：SAP 哈希模块
//! - [`hand_garble`]：复杂的字节混淆模块

// FairPlay 算法是逆向产物，许多代码看起来"笨拙"或"无意义"，但这是有意的。
// 关闭 clippy 以避免 lint 干扰。
#![allow(clippy::all)]
#![allow(clippy::pedantic)]

pub mod consts;
pub mod hand_garble;
pub mod modified_md5;
pub mod omg_hax;
pub mod sap_hash;
