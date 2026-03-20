#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod config;
mod hotkey;
mod ui;

use anyhow::{Context, Result};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, TranslateMessage,
};

static IS_CAPTURING: AtomicBool = AtomicBool::new(false);
static CTRL_ALT_L: LazyLock<HotKey> =
    LazyLock::new(|| HotKey::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyL));

fn main() -> Result<()> {
    let config = config::load_config().with_context(|| "配置加载失败")?;
    let _manager = hotkey::register_hotkey(config)?; // 仅仅是让manager存在于处理windows消息循环的线程内。

    unsafe {
        // 去读取底层消息，以便让register_hotkey内的 receiver.recv() 才能拿到 Pressed 状态
        let mut msg: MSG = std::mem::zeroed();
        // 阻塞，等待消息
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}
