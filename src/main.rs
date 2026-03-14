#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod config;
mod ui;

use anyhow::Result;
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager,
    hotkey::{Code, HotKey, Modifiers},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, TranslateMessage,
};

static IS_CAPTURING: AtomicBool = AtomicBool::new(false);

fn main() -> Result<()> {
    let config = config::load_config().unwrap_or_else(|e| {
        eprintln!("配置加载失败: {}", e);
        std::process::exit(1);
    });

    let config_arc = Arc::new(config);
    let manager = GlobalHotKeyManager::new()?;
    let hotkey = HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyL);
    manager.register(hotkey).unwrap();
    println!("热键注册成功！请按 Ctrl + Alt + L 开始截屏。");

    let receiver = GlobalHotKeyEvent::receiver();
    
    thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            if event.state == global_hotkey::HotKeyState::Pressed {
                if IS_CAPTURING
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    let thread_config = Arc::clone(&config_arc);
                    thread::spawn(move || {
                        if let Err(e) = ui::run_capture_ui_and_ocr(&thread_config) {
                            eprintln!("工作流失败: {}", e);
                        }
                        IS_CAPTURING.store(false, Ordering::SeqCst);
                    });
                }
            }
        }
    });

    unsafe {  // 去读取底层消息，你前面写的 receiver.recv() 才能拿到 Pressed 状态
        let mut msg: MSG = std::mem::zeroed();
        // 阻塞，等待消息
        while GetMessageW(&mut msg, 0 as _, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}
