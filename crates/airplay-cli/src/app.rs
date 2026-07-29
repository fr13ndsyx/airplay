//! egui 主窗口 —— AirPlay 控制面板。
//!
//! 状态显示（颜色区分）+ 启动/停止按钮 + iPhone 操作提示。
//! 关闭窗口拦截：隐藏到托盘而非退出。

use std::sync::atomic::Ordering;

use eframe::egui;

use crate::status::{AppStatus, CmdTx, ServerCommand, StatusRx};
use crate::tray::{QUIT_REQUESTED, SHOW_WINDOW};

/// egui 应用主结构。
pub struct AirPlayApp {
    status_rx: StatusRx,
    cmd_tx: CmdTx,
    window_visible: bool,
    volume: f32,
}

impl AirPlayApp {
    /// 创建应用实例。
    ///
    /// 加载 Windows 系统中文字体（微软雅黑），解决中文乱码问题。
    pub fn new(cc: &eframe::CreationContext, status_rx: StatusRx, cmd_tx: CmdTx) -> Self {
        setup_chinese_fonts(&cc.egui_ctx);
        Self {
            status_rx,
            cmd_tx,
            window_visible: true,
            volume: 1.0,
        }
    }
}

/// 加载中文字体（微软雅黑），解决 egui 默认字体不含中文的问题。
///
/// 优先级：C:\Windows\Fonts\msyh.ttc（微软雅黑）
/// 回退：C:\Windows\Fonts\simsun.ttc（宋体）
fn setup_chinese_fonts(ctx: &egui::Context) {
    let font_paths = [
        "C:\\Windows\\Fonts\\msyh.ttc", // 微软雅黑
        "C:\\Windows\\Fonts\\msyhbd.ttc", // 微软雅黑粗体
        "C:\\Windows\\Fonts\\simhei.ttf", // 黑体
        "C:\\Windows\\Fonts\\simsun.ttc", // 宋体
    ];

    let mut fonts = egui::FontDefinitions::default();

    for path in &font_paths {
        if let Ok(font_data) = std::fs::read(path) {
            let family_name = path
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or("font")
                .trim_end_matches(".ttc")
                .trim_end_matches(".ttf");
            tracing::info!("加载中文字体: {} ({})", family_name, path);

            fonts.font_data.insert(
                family_name.to_owned(),
                egui::FontData::from_owned(font_data).into(),
            );

            // 将中文字体设为 Proportional 和 Monospace 的首选
            if let Some(proportional) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                proportional.insert(0, family_name.to_owned());
            }
            if let Some(monospace) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                monospace.push(family_name.to_owned());
            }
            break;
        }
    }

    ctx.set_fonts(fonts);
}

impl eframe::App for AirPlayApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 检查退出请求（来自托盘"退出"菜单）
        if QUIT_REQUESTED.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // 检查"显示窗口"请求（来自托盘点击或"显示窗口"菜单）
        if SHOW_WINDOW.swap(false, Ordering::SeqCst) {
            if !self.window_visible {
                self.window_visible = true;
                // 把窗口移回屏幕内
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::Pos2::new(100.0, 100.0)));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
        }

        // 拦截窗口关闭：改为隐藏到托盘
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.window_visible {
                // 把窗口移到屏幕外，保持事件循环运行
                // Visible(false) 和 Minimized(true) 都会停止事件循环导致托盘事件无法唤醒
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::Pos2::new(-10000.0, -10000.0)));
                self.window_visible = false;
            }
        }

        // 读取最新状态（非阻塞 borrow）
        let status = self.status_rx.borrow().clone();

        // 渲染 UI
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("airplay-rs");
            ui.separator();

            // 状态显示（颜色区分）+ 控制按钮
            match &status {
                AppStatus::Stopped => {
                    ui.colored_label(egui::Color32::GRAY, "● 已停止");
                    if ui.button("启动服务").clicked() {
                        let _ = self.cmd_tx.send(ServerCommand::Start);
                    }
                }
                AppStatus::Starting => {
                    ui.colored_label(egui::Color32::YELLOW, "● 启动中...");
                }
                AppStatus::Running { port } => {
                    ui.colored_label(
                        egui::Color32::GREEN,
                        format!("● 等待连接 (端口 {})", port),
                    );
                    if ui.button("停止服务").clicked() {
                        let _ = self.cmd_tx.send(ServerCommand::Stop);
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("音量:");
                        let mut vol = self.volume;
                        let slider = egui::Slider::new(&mut vol, 0.0..=1.0)
                            .suffix(" 🔊");
                        if ui.add(slider).changed() {
                            self.volume = vol;
                            let _ = self.cmd_tx.send(ServerCommand::SetVolume(vol));
                        }
                        if ui.button("🔇 静音").clicked() {
                            self.volume = 0.0;
                            let _ = self.cmd_tx.send(ServerCommand::SetVolume(0.0));
                        }
                    });
                }
                AppStatus::Connected => {
                    ui.colored_label(
                        egui::Color32::from_rgb(0, 200, 255),
                        "● iPhone 已连接，正在投屏",
                    );
                    if ui.button("停止服务").clicked() {
                        let _ = self.cmd_tx.send(ServerCommand::Stop);
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("音量:");
                        let mut vol = self.volume;
                        let slider = egui::Slider::new(&mut vol, 0.0..=1.0)
                            .suffix(" 🔊");
                        if ui.add(slider).changed() {
                            self.volume = vol;
                            let _ = self.cmd_tx.send(ServerCommand::SetVolume(vol));
                        }
                        if ui.button("🔇 静音").clicked() {
                            self.volume = 0.0;
                            let _ = self.cmd_tx.send(ServerCommand::SetVolume(0.0));
                        }
                    });
                }
                AppStatus::Disconnected { port } => {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        format!("● iPhone 已断开 (端口 {})", port),
                    );
                    if ui.button("停止服务").clicked() {
                        let _ = self.cmd_tx.send(ServerCommand::Stop);
                    }
                }
                AppStatus::Error(msg) => {
                    ui.colored_label(egui::Color32::RED, format!("● 错误: {}", msg));
                    if ui.button("启动服务").clicked() {
                        let _ = self.cmd_tx.send(ServerCommand::Start);
                    }
                }
            }

            ui.separator();
            ui.label("iPhone 操作：");
            ui.label("1. 确保 iPhone 与电脑在同一 Wi-Fi");
            ui.label("2. 打开控制中心 → 屏幕镜像");
            ui.label("3. 选择 'airplay-rs-mirror'");

            ui.separator();
            ui.label("关闭此窗口将最小化到托盘");
            ui.label("右键托盘图标可显示窗口或退出");
        });

        // 持续重绘以更新状态（每 200ms 请求一次重绘）
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }
}
