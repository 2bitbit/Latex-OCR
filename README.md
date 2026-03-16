# Latex-OCR

Windows 平台的一个基于 Rust 开发、超轻量级(后台静默时内存占用<1MB)、常驻后台、全局快捷键截屏公式识别工具。

通过调用最新多模态大模型的 API，本项目可以一键将屏幕上的数学公式截图转换为纯文本的 LaTeX 源码，并自动写入系统剪贴板。

## 核心特性

* **极致内存控制：** 在后台常驻时几乎不占用额外的 CPU 和内存资源。
* **原生截屏体验：** 等效于 `Win + Shift + S`。
* **零磁盘 I/O 损耗：** 截屏后，自动直接从系统剪贴板读取原始像素数据，完全在内存中完成 PNG 压缩与 Base64 编码，无需生成任何临时本地文件。
* **高度可配置：** 支持任何兼容 OpenAI 标准格式的大模型 API（如 OpenAI、火山引擎、DeepSeek 等），无需重新编译即可动态切换模型。

## 快速配置与使用

### 1. 配置文件定义
点击下载 [Latex-OCR](https://github.com/2bitbit/Latex-OCR/releases/latest)，在 `Latex-OCR.exe` 同级目录下，必须创建一个名为 `config.txt` 的文本文件。程序将在启动时自动读取该配置。

**`config.txt` 配置模板：**

```ini
# API_KEY：填写您的鉴权密钥
API_KEY=your_api_key_here
# API_URL：填写完整的 API 路由地址（注意：必须精确到 /v1/chat/completions 或 /api/v3/chat/completions）
API_URL=https://ark.cn-beijing.volces.com/api/v3/chat/completions
# MODEL：填写调用的模型名称或实际推理接入点 ID (Endpoint ID)
MODEL=doubao-seed-2-0-mini-260215
```

### 2. 操作流程
> [!IMPORTANT]
> 程序启动后即已经在后台运行，不要重复双击启动程序。（重复点击也没事的。）

1. 双击运行 `Latex-OCR.exe`，程序将在后台启动并开启键盘监听。
2. 在任意界面，按下全局快捷键组合：**`Ctrl + Alt + L`**。（在获取上一次公式识别结果前，此快捷键是暂时无效的。）
3. 屏幕将自动变暗，使用鼠标左键框选需要识别的公式区域。
4. 松开鼠标后，程序会自动处理截图并发送给大模型。（并清空系统剪贴板）
5. 稍等片刻，识别出的纯净 LaTeX 代码将被自动复制到您的系统剪贴板中。
6. 直接在 Word、Markdown 或 LaTeX 编辑器中 `Ctrl + V` 粘贴即可。

### 3. 进阶配置
配置开机自启动：打开Task Scheduler，创建一个新任务，设置触发器为登录时，操作选择启动程序，程序路径为 `Latex-OCR.exe`，即可实现开机自启动。
