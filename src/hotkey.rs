use crate::ui;
use crate::{CTRL_ALT_L, IS_CAPTURING};

use anyhow::{Context, Result};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;

pub fn register_hotkey(config: HashMap<String, String>) -> Result<GlobalHotKeyManager> {
    let manager = GlobalHotKeyManager::new()?;
    manager
        .register(*CTRL_ALT_L)
        .with_context(|| "热键注册失败")?;
    println!("热键注册成功！请按 Ctrl + Alt + L 开始截屏。");

    // 用于监听热键的新线程。
    thread::spawn(move || -> Result<()> {
        let receiver = GlobalHotKeyEvent::receiver();
        let thread_config = Arc::new(config);
        while let Ok(event) = receiver.recv() {
            if event.state == global_hotkey::HotKeyState::Pressed {
                if IS_CAPTURING
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    if let Err(e) = ui::run_capture_ui_and_ocr(&thread_config) {
                        eprintln!("工作流失败: {:?}", e);
                    }
                    IS_CAPTURING.store(false, Ordering::SeqCst);
                }
            }
        }
        Ok(())
    });

    Ok(manager)
}
