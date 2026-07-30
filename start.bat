@echo off
REM ============================================================
REM   airplay-rs 启动脚本（CLI 模式）
REM ============================================================
REM   双击即可启动；按 Ctrl+C 退出。
REM   投屏画面会在独立的 GStreamer 原生窗口中显示。
REM ============================================================

REM 切换到脚本所在目录（确保相对路径正确）
cd /d "%~dp0"

REM 设置日志级别（可选：debug / info / warn / error）
REM set RUST_LOG=debug

REM 运行编译好的二进制（优先 release，回退 debug）
if exist "target\release\airplay-cli.exe" (
    "target\release\airplay-cli.exe"
) else if exist "target\debug\airplay-cli.exe" (
    "target\debug\airplay-cli.exe"
) else (
    echo [错误] 未找到 airplay-cli.exe
    echo 请先运行以下命令编译：
    echo     cargo build --release
    echo 或  cargo build
    echo.
    pause
    exit /b 1
)

REM 异常退出时暂停，便于查看错误信息
if %ERRORLEVEL% neq 0 (
    echo.
    echo [程序异常退出，代码 %ERRORLEVEL%]
    pause
)
