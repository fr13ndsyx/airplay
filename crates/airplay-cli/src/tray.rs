//! 系统托盘 —— tray-icon 0.14 + muda 菜单。
//!
//! 创建托盘图标和右键菜单，菜单事件通过单独线程 poll。
//! 收到事件后通过 `cmd_tx` 发送命令给 server task。
//!
//! 通过全局 `AtomicBool` 标志与 eframe 主循环通信：
//! - `SHOW_WINDOW`：通知 eframe 显示主窗口
//! - `QUIT_REQUESTED`：通知 eframe 退出
//!
//! `EGUI_CTX`：存储 egui Context，托盘点击时主动触发重绘，
//!             因为 `Visible(false)` 隐藏窗口后 eframe 不会再调用 update()。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result};
use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::status::{CmdTx, ServerCommand};

/// 通知 eframe 显示主窗口的全局标志。
pub static SHOW_WINDOW: AtomicBool = AtomicBool::new(true);
/// 通知 eframe 退出的全局标志。
pub static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);
/// 存储 egui Context，托盘点击时主动触发重绘。
/// 窗口 `Visible(false)` 隐藏后 eframe 不再调用 update()，
/// 所以必须由托盘线程主动 `request_repaint()` 唤醒。
pub static EGUI_CTX: Mutex<Option<egui::Context>> = Mutex::new(None);

/// 由 app.rs 在初始化时调用，存储 egui Context。
pub fn set_egui_ctx(ctx: egui::Context) {
    *EGUI_CTX.lock().unwrap() = Some(ctx);
}

/// 托盘状态。持有 TrayIcon 和菜单项保持其存活。
pub struct TrayState {
    _tray_icon: TrayIcon,
    _show_item: MenuItem,
    _start_item: MenuItem,
    _stop_item: MenuItem,
    _quit_item: MenuItem,
    _sep1: PredefinedMenuItem,
    _sep2: PredefinedMenuItem,
    _event_thread: std::thread::JoinHandle<()>,
}

impl TrayState {
    /// 创建系统托盘。
    pub fn new(cmd_tx: CmdTx) -> Result<Self> {
        let icon = create_airplay_icon().context("创建托盘图标失败")?;

        // 构建菜单
        let menu = Menu::new();
        let show_item = MenuItem::with_id("show", "显示窗口", true, None);
        let start_item = MenuItem::with_id("start", "启动服务", true, None);
        let stop_item = MenuItem::with_id("stop", "停止服务", true, None);
        let quit_item = MenuItem::with_id("quit", "退出", true, None);

        let sep1 = PredefinedMenuItem::separator();
        let sep2 = PredefinedMenuItem::separator();

        menu.append(&show_item)?;
        menu.append(&sep1)?;
        menu.append(&start_item)?;
        menu.append(&stop_item)?;
        menu.append(&sep2)?;
        menu.append(&quit_item)?;

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("airplay-rs")
            .with_icon(icon)
            .build()
            .context("构建 TrayIcon 失败")?;

        // spawn 单独线程 poll 菜单事件 + 托盘点击事件
        let event_thread = std::thread::Builder::new()
            .name("tray-event".into())
            .spawn(move || {
                let menu_receiver = MenuEvent::receiver();
                let tray_receiver = TrayIconEvent::receiver();
                loop {
                    // poll 菜单事件
                    if let Ok(event) = menu_receiver.try_recv() {
                        handle_menu_event(&event, &cmd_tx);
                    }
                    // poll 托盘图标点击事件
                    if let Ok(event) = tray_receiver.try_recv() {
                        handle_tray_event(&event);
                    }
                    // 检查退出
                    if QUIT_REQUESTED.load(Ordering::SeqCst) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            })?;

        Ok(Self {
            _tray_icon: tray_icon,
            _show_item: show_item,
            _start_item: start_item,
            _stop_item: stop_item,
            _quit_item: quit_item,
            _sep1: sep1,
            _sep2: sep2,
            _event_thread: event_thread,
        })
    }
}

/// 处理菜单事件。
fn handle_menu_event(event: &MenuEvent, cmd_tx: &CmdTx) {
    let id = event.id();
    match id.as_ref() {
        "show" => {
            request_show_window();
        }
        "start" => {
            let _ = cmd_tx.send(ServerCommand::Start);
        }
        "stop" => {
            let _ = cmd_tx.send(ServerCommand::Stop);
        }
        "quit" => {
            QUIT_REQUESTED.store(true, Ordering::SeqCst);
            let _ = cmd_tx.send(ServerCommand::Shutdown);
            // 退出时也要唤醒 eframe 让它检测到 QUIT_REQUESTED
            wake_egui();
        }
        other => {
            tracing::debug!("未知菜单事件: {}", other);
        }
    }
}

/// 处理托盘图标点击事件（单击显示窗口）。
fn handle_tray_event(event: &TrayIconEvent) {
    if let TrayIconEvent::Click { .. } = event {
        request_show_window();
    }
}

/// 设置 SHOW_WINDOW 标志并主动唤醒 eframe 重绘。
///
/// 关键：窗口 `Visible(false)` 后 eframe 会停止事件循环，
/// `request_repaint()` 无法唤醒它。所以必须直接发送 `Visible(true)`
/// viewport 命令，让 winit 重新激活窗口和事件循环。
fn request_show_window() {
    SHOW_WINDOW.store(true, Ordering::SeqCst);
    wake_egui();
}

/// 通过存储的 egui::Context 触发重绘，唤醒被最小化的窗口。
/// Minimized(true) 不会停止事件循环，所以 request_repaint 能正常工作。
fn wake_egui() {
    if let Ok(guard) = EGUI_CTX.lock() {
        if let Some(ctx) = guard.as_ref() {
            ctx.request_repaint();
        }
    }
}

/// 生成 AirPlay 风格图标（64x64）。
///
/// 设计：深蓝灰色圆角背景 + 白色 TV 矩形外框 + 白色向上三角形。
fn create_airplay_icon() -> Result<Icon> {
    let width: u32 = 64;
    let height: u32 = 64;
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    let put_pixel = |rgba: &mut [u8], x: u32, y: u32, w: u32, r: u8, g: u8, b: u8, a: u8| {
        if x < w && y < height {
            let idx = ((y * w + x) * 4) as usize;
            rgba[idx] = r;
            rgba[idx + 1] = g;
            rgba[idx + 2] = b;
            rgba[idx + 3] = a;
        }
    };

    // 1. 深蓝灰色圆角矩形背景
    let bg = (30u8, 30u8, 45u8, 255u8);
    let margin = 4;
    let radius = 12;
    for y in 0..height {
        for x in 0..width {
            // 圆角检测
            let in_corner = (x < margin + radius && y < margin + radius)
                || (x >= width - margin - radius && y < margin + radius)
                || (x < margin + radius && y >= height - margin - radius)
                || (x >= width - margin - radius && y >= height - margin - radius);

            let fill = if in_corner {
                let cx = if x < margin + radius {
                    margin + radius
                } else {
                    width - margin - radius - 1
                };
                let cy = if y < margin + radius {
                    margin + radius
                } else {
                    height - margin - radius - 1
                };
                let dx = x as i64 - cx as i64;
                let dy = y as i64 - cy as i64;
                dx * dx + dy * dy <= (radius as i64) * (radius as i64)
            } else {
                x >= margin && x < width - margin && y >= margin && y < height - margin
            };

            if fill {
                put_pixel(&mut rgba, x, y, width, bg.0, bg.1, bg.2, bg.3);
            }
        }
    }

    // 2. TV 外框（白色矩形边框，底部留空给三角形）
    let white = (240u8, 240u8, 240u8, 255u8);
    let tv_x1 = 14;
    let tv_y1 = 18;
    let tv_x2 = width - 14; // 50
    let tv_y2 = 42;
    let thickness = 3;
    for y in tv_y1..=tv_y2 {
        for x in tv_x1..=tv_x2 {
            let on_edge = x < tv_x1 + thickness
                || x > tv_x2.saturating_sub(thickness)
                || y < tv_y1 + thickness
                || y > tv_y2.saturating_sub(thickness);
            if on_edge {
                put_pixel(&mut rgba, x, y, width, white.0, white.1, white.2, white.3);
            }
        }
    }

    // 3. AirPlay 三角形（指向上的等腰三角形，位于 TV 下方）
    let tri_top_x = width / 2; // 32
    let tri_top_y = 50;
    let tri_bl_x = width / 2 - 10; // 22
    let tri_bl_y = 56;
    let tri_br_x = width / 2 + 10; // 42
    let tri_br_y = 56;

    for y in tri_top_y..=tri_bl_y {
        for x in tri_bl_x..=tri_br_x {
            if point_in_triangle(
                x as i64,
                y as i64,
                tri_top_x as i64,
                tri_top_y as i64,
                tri_bl_x as i64,
                tri_bl_y as i64,
                tri_br_x as i64,
                tri_br_y as i64,
            ) {
                put_pixel(&mut rgba, x, y, width, white.0, white.1, white.2, white.3);
            }
        }
    }

    Icon::from_rgba(rgba, width, height)
        .map_err(|e| anyhow::anyhow!("Icon::from_rgba 失败: {}", e))
}

/// 点在三角形内判定。
fn point_in_triangle(
    px: i64,
    py: i64,
    x1: i64,
    y1: i64,
    x2: i64,
    y2: i64,
    x3: i64,
    y3: i64,
) -> bool {
    let d = (y2 - y3) * (x1 - x3) + (x3 - x2) * (y1 - y3);
    if d == 0 {
        return false;
    }
    let a = ((y2 - y3) * (px - x3) + (x3 - x2) * (py - y3)) as f64 / d as f64;
    let b = ((y3 - y1) * (px - x3) + (x1 - x3) * (py - y3)) as f64 / d as f64;
    let c = 1.0 - a - b;
    a >= 0.0 && b >= 0.0 && c >= 0.0
}
