#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Context, Result, anyhow};
use arboard::Clipboard;
use base64::prelude::*;
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager,
    hotkey::{Code, HotKey, Modifiers},
};
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use screenshots::Screen;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, TranslateMessage,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, WS_CAPTION, WS_THICKFRAME,
};

// 禁用系统 DPI 缩放，强制物理像素 1:1 映射
// 并引入底层窗口样式修改 API，用于彻底干掉白条
#[link(name = "user32")]
unsafe extern "system" {
    unsafe fn SetProcessDPIAware() -> i32;
    unsafe fn GetWindowLongW(hWnd: isize, nIndex: i32) -> u32;
    unsafe fn SetWindowLongW(hWnd: isize, nIndex: i32, dwNewLong: u32) -> i32;
    unsafe fn SetWindowPos(
        hWnd: isize,
        hWndInsertAfter: isize,
        X: i32,
        Y: i32,
        cx: i32,
        cy: i32,
        uFlags: u32,
    ) -> i32;
}

static IS_CAPTURING: AtomicBool = AtomicBool::new(false);

fn main() -> Result<()> {
    unsafe {
        SetProcessDPIAware();
    }

    let config = load_config().unwrap_or_else(|e| {
        eprintln!("配置加载失败: {}", e);
        std::process::exit(1);
    });
    let config_arc = Arc::new(config);

    let manager = GlobalHotKeyManager::new()?;
    let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyL);
    manager.register(hotkey).unwrap();
    println!("热键注册成功！请按 Ctrl + Alt + L 开始截屏。");

    let receiver = GlobalHotKeyEvent::receiver();

    let config_arc_thread = Arc::clone(&config_arc);
    thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            if event.state == global_hotkey::HotKeyState::Pressed {
                if IS_CAPTURING
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    let thread_config = Arc::clone(&config_arc_thread);
                    thread::spawn(move || {
                        if let Err(e) = run_capture_ui_and_ocr(&thread_config) {
                            eprintln!("工作流失败: {}", e);
                        }
                        IS_CAPTURING.store(false, Ordering::SeqCst);
                    });
                }
            }
        }
    });

    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0 as _, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}

fn run_capture_ui_and_ocr(config: &HashMap<String, String>) -> Result<()> {
    // 1. 抓取屏幕并提取所有像素点
    let screens = Screen::all().with_context(|| "无法获取显示器列表")?;
    let screen = screens.first().with_context(|| "没有找到屏幕")?;
    let capture = screen.capture().with_context(|| "抓图失败")?;

    let width = capture.width() as usize;
    let height = capture.height() as usize;
    let raw_pixels = capture.into_raw();

    // 2. 准备底层画布：高亮原图与全局变暗的底图
    let mut original_bg = vec![0u32; width * height];
    let mut dark_bg = vec![0u32; width * height];

    for (i, chunk) in raw_pixels.chunks_exact(4).enumerate() {
        let r = chunk[0] as u32;
        let g = chunk[1] as u32;
        let b = chunk[2] as u32;

        // minifb 使用 0RGB 格式
        let color = (r << 16) | (g << 8) | b;
        original_bg[i] = color;

        // RGB 值折半，实现直接的变暗效果
        let dark_color = ((r / 2) << 16) | ((g / 2) << 8) | (b / 2);
        dark_bg[i] = dark_color;
    }

    // 3. 打开纯粹的像素推送窗口
    let mut window = Window::new(
        "OCR Screen Selector",
        width,
        height,
        WindowOptions {
            borderless: true,
            title: false,
            topmost: true,
            resize: false,
            ..WindowOptions::default()
        },
    )
    .unwrap();
    // --- 🌟 彻底干掉白条的核心代码开始 ---
    let hwnd = window.get_window_handle() as isize;
    unsafe {
        // 获取当前窗口的底层样式
        let style = GetWindowLongW(hwnd, GWL_STYLE);
        // 强行剔除标题栏 (WS_CAPTION) 和可调边框 (WS_THICKFRAME)
        SetWindowLongW(hwnd, GWL_STYLE, style & !WS_CAPTION & !WS_THICKFRAME);
        // 通知 Windows 刷新窗口样式，使其瞬间生效
        SetWindowPos(
            hwnd,
            0,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
        );
    }
    // --- 🌟 彻底干掉白条的核心代码结束 ---
    // 强制窗口对齐主屏幕左上角
    window.set_position(0, 0);
    window.set_target_fps(120); // 设置刷新率约 120FPS，丝滑就完事了！

    let mut start_pos: Option<(f32, f32)> = None;
    let mut end_pos: Option<(f32, f32)> = None;
    let mut is_selecting = false;
    let mut final_rect = None;

    let mut current_buffer = dark_bg.clone();

    // 4. 极简的交互循环
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let mouse_pos = window.get_mouse_pos(MouseMode::Clamp);
        let left_down = window.get_mouse_down(MouseButton::Left);

        if left_down {
            if !is_selecting {
                start_pos = mouse_pos;
                is_selecting = true;
            }
            end_pos = mouse_pos;
        } else {
            if is_selecting {
                // 鼠标松开，确定最终框选坐标并退出循环
                if let (Some(s), Some(e)) = (start_pos, end_pos) {
                    let rx = s.0.min(e.0) as u32;
                    let ry = s.1.min(e.1) as u32;
                    let rw = (s.0 - e.0).abs() as u32;
                    let rh = (s.1 - e.1).abs() as u32;
                    if rw > 5 && rh > 5 {
                        final_rect = Some((rx, ry, rw, rh));
                    }
                }
                break;
            }
        }

        // --- 画面重绘逻辑 ---
        if is_selecting {
            // 每帧先铺满暗色背景
            current_buffer.copy_from_slice(&dark_bg);

            if let (Some(s), Some(e)) = (start_pos, end_pos) {
                let rx = s.0.min(e.0) as usize;
                let ry = s.1.min(e.1) as usize;
                let rw = (s.0 - e.0).abs() as usize;
                let rh = (s.1 - e.1).abs() as usize;

                // 核心：把框选区域的像素替换为原始高亮像素
                for y in ry..=(ry + rh).min(height - 1) {
                    let row_idx = y * width;
                    let start_idx = row_idx + rx;
                    let end_idx = row_idx + (rx + rw).min(width - 1);
                    current_buffer[start_idx..=end_idx]
                        .copy_from_slice(&original_bg[start_idx..=end_idx]);
                }

                // 画出 2px 宽度的纯绿边框
                let border_color = 0x00_00_FF_00;
                let border_thickness = 2;
                for y in ry..=(ry + rh).min(height - 1) {
                    for x in rx..=(rx + rw).min(width - 1) {
                        let is_top = y < ry + border_thickness;
                        let is_bottom = y > ry + rh - border_thickness;
                        let is_left = x < rx + border_thickness;
                        let is_right = x > rx + rw - border_thickness;

                        if is_top || is_bottom || is_left || is_right {
                            current_buffer[y * width + x] = border_color;
                        }
                    }
                }
            }
        } else {
            current_buffer.copy_from_slice(&dark_bg);
        }

        window
            .update_with_buffer(&current_buffer, width, height)
            .unwrap();
    }

    // 5. 交互结束，直接强制销毁窗口
    drop(window);

    // 6. 裁剪最终图像并发往 API
    if let Some((x, y, w, h)) = final_rect {
        println!("选区坐标: x:{}, y:{}, w:{}, h:{}", x, y, w, h);

        let mut rgba_image = image::RgbaImage::from_raw(width as u32, height as u32, raw_pixels)
            .with_context(|| "解析像素失败")?;

        let cropped = image::imageops::crop(&mut rgba_image, x, y, w, h).to_image();

        send_to_api(config, cropped)?;
    } else {
        println!("取消截图或选区过小。");
    }

    Ok(())
}

fn send_to_api(config: &HashMap<String, String>, img: image::RgbaImage) -> Result<()> {
    let mut png_buffer = Cursor::new(Vec::new());
    img.write_to(&mut png_buffer, image::ImageFormat::Png)?;
    let b64_string = BASE64_STANDARD.encode(png_buffer.into_inner());

    let api_key = config
        .get("API_KEY")
        .with_context(|| "错误: API_KEY 必须存在")?;
    let api_url = config
        .get("API_URL")
        .with_context(|| "错误: API_URL 必须存在")?;
    let model_name = config
        .get("MODEL")
        .with_context(|| "错误: MODEL 必须存在")?;

    let payload = serde_json::json!({
        "model": model_name,
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": "Please output ONLY the raw LaTeX code for the math formulas in this image. Do NOT use any formatting or wrappers."
                    },
                    {
                        "type": "image_url",
                        "image_url": { "url": format!("data:image/png;base64,{}", b64_string) }
                    }
                ]
            }
        ]
    });

    println!("正在调用大模型进行识别...");
    let payload_str = serde_json::to_string(&payload)?;
    let mut response = ureq::post(api_url)
        .header("Authorization", &format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .send(payload_str)?;

    let response_text = response.body_mut().read_to_string()?;
    let response_json: serde_json::Value = serde_json::from_str(&response_text)?;

    if let Some(content) = response_json["choices"][0]["message"]["content"].as_str() {
        let latex_code = content.trim();
        println!("识别完成！\n{}", latex_code);
        let mut clipboard = Clipboard::new()?;
        clipboard.set_text(latex_code)?;
        println!("==== 已成功复制到剪贴板 ====");
    } else {
        eprintln!("API 响应格式异常: {}", response_text);
    }

    Ok(())
}

fn load_config() -> Result<HashMap<String, String>> {
    let mut exe_path = env::current_exe()?;
    exe_path.pop();
    exe_path.push("config.txt");
    let config_content = fs::read_to_string(&exe_path)
        .with_context(|| format!("无法在 {} 找到 config.txt", exe_path.display()))?;

    let mut config_map = HashMap::new();
    for line in config_content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            config_map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    Ok(config_map)
}
