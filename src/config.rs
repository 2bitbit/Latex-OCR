use anyhow::{Context, Result, bail};
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
        match line.split_once('=') {
            Some((key, value)) => {
                let key_trimmed = key.trim();
                if key_trimmed.ne("API_KEY") && key_trimmed.ne("API_URL") && key_trimmed.ne("MODEL")
                {
                    anyhow::bail!("配置项 {} 不存在", key_trimmed);
                }
                let value_trimmed = value.trim();
                if value_trimmed.is_empty() {
                    anyhow::bail!("{} 的配置值不能为空", key_trimmed);
                }
                config_map.insert(key_trimmed.to_string(), value_trimmed.to_string());
            }
            None => {
                anyhow::bail!("无法解析的配置行: {}", line);
            }
        }
    }
    if config_map.len() != 3 {
        bail!("配置文件格式错误，必须包含 API_KEY、API_URL 和 MODEL 三项配置");
    }
    Ok(config_map)
}
