use anyhow::{Context, Result};
use arboard::Clipboard;
use base64::prelude::*;
use std::collections::HashMap;
use std::io::Cursor;

pub fn send_to_api_and_override_clipboard(
    config: &HashMap<String, String>,
    img: image::RgbaImage,
) -> Result<()> {
    let mut clipboard = Clipboard::new()?;
    clipboard.clear().with_context(|| "清空剪贴板失败")?;

    let mut png_buffer = Cursor::new(Vec::new()); //将一维内存数组包装成一个符合标准 I/O 规范的虚拟流对象
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

    let response_text = response.body_mut().read_to_string()?; // 因为是边读边复用，所以必须是可变借用！？
    let response_json: serde_json::Value = serde_json::from_str(&response_text)?;

    match response_json["choices"][0]["message"]["content"].as_str() {
        Some(content) => {
            let latex_code = content.trim();
            println!("识别完成！\n{}", latex_code);

            clipboard.set_text(latex_code)?;
            println!("==== 已成功复制到剪贴板 ====");
            Ok(())
        }

        None => {
            anyhow::bail!("API 响应格式异常: {}", response_text);
        }
    }
}
