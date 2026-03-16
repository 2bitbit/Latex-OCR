use crate::api::send_to_api;
use anyhow::{Context, Result};
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use std::collections::HashMap;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetWindowLongW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SetForegroundWindow,
    SetProcessDPIAware, SetWindowLongW, SetWindowPos, WS_CAPTION, WS_THICKFRAME,
};
use xcap::Monitor;

pub fn run_capture_ui_and_ocr(config: &HashMap<String, String>) -> Result<()> {
    unsafe {
        SetProcessDPIAware();
    }

    // 1. 抓取屏幕并提取所有像素点
    let monitors = Monitor::all().with_context(|| "无法获取显示器列表")?;
    let monitor = monitors.first().with_context(|| "没有找到屏幕")?;
    let image = monitor.capture_image().with_context(|| "抓图失败")?;

    let width = image.width() as usize;
    let height = image.height() as usize;
    let raw_pixels = image.into_raw();

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
    let hwnd = window.get_window_handle() as *mut std::ffi::c_void;
    unsafe {
        // 获取当前窗口的底层样式
        let style = GetWindowLongW(hwnd, GWL_STYLE);
        // 强行剔除标题栏 (WS_CAPTION) 和可调边框 (WS_THICKFRAME)
        SetWindowLongW(
            hwnd,
            GWL_STYLE,
            ((style as u32) & !WS_CAPTION & !WS_THICKFRAME) as i32,
        );
        // 通知 Windows 刷新窗口样式，使其瞬间生效
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
        );

        // 强行把新创建的窗口置于前台并激活
        // 从而确保立即获得键盘焦点，这样第一时间按 Esc 就起效
        SetForegroundWindow(hwnd);
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

                // 上边缘和下边缘
                for y in 0..border_thickness {
                    if ry + y < height {
                        let row_top = (ry + y) * width;
                        for x in rx..=(rx + rw).min(width - 1) {
                            current_buffer[row_top + x] = border_color;
                        }
                    }
                    if ry + rh >= y && ry + rh - y < height {
                        let row_bottom = (ry + rh - y) * width;
                        for x in rx..=(rx + rw).min(width - 1) {
                            current_buffer[row_bottom + x] = border_color;
                        }
                    }
                }

                // 左边缘和右边缘
                for x in 0..border_thickness {
                    if rx + x < width {
                        for y in ry..=(ry + rh).min(height - 1) {
                            current_buffer[y * width + rx + x] = border_color;
                        }
                    }
                    if rx + rw >= x && rx + rw - x < width {
                        for y in ry..=(ry + rh).min(height - 1) {
                            current_buffer[y * width + rx + rw - x] = border_color;
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
