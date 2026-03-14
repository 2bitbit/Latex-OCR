use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::fs;

/// 加载配置文件
pub fn load_config() -> Result<HashMap<String, String>> {
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
