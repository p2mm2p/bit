//! bit —— 两个交互命令的 git 包装，不做透传。
//!
//! 分层：本文件与子模块放纯函数、数据与可单测的 AI 供给客户端；`main.rs` 只做解析与调度。
//! 命令面（帮助正文、诊断文案、三层退出码）冻结在
//! [命令面 · 帮助、错误与文案语言](https://github.com/p2mm2p/bit/issues/8)，
//! 决策依据见 [ADR-0003](../../docs/adr/0003-bit-owns-its-command-surface.md)。

pub mod ai;
pub mod branch;
pub mod cli;
pub mod commit;
pub mod config;
pub mod login;
