<p align="center">
  <h1 align="center">OpenCrab 🦀</h1>
  <p align="center">用 Rust 构建的多通道 AI 网关 · Multi-channel AI Gateway</p>
  <p align="center">
    <strong>23 种 LLM</strong> · <strong>飞书 Bot</strong> · <strong>WebSocket API</strong> · <strong>单二进制 14 MB</strong>
  </p>
</p>

---

## 特性

- **23 种 LLM Provider** — OpenAI / Anthropic / DeepSeek / 智谱 / Ollama / Groq / Gemini … 一行配置切换
- **飞书 Bot** — WebSocket 长连接，无需公网 IP，私聊 + 群聊 @Bot
- **Gateway API** — HTTP 健康检查 + WebSocket JSON-RPC，方便前端 / 脚本集成
- **CLI 直聊** — `opencrab chat "你好"` 无需启动 Gateway
- **内置工具** — Agent 可调用 `bash`、`read_file` 完成任务
- **TOML 配置** — 支持 `${ENV_VAR}` 环境变量替换
- **单二进制** — `cargo build --release && strip` 仅 14 MB

---

## 快速开始

### 1. 构建

```bash
cargo build --release
strip target/release/opencrab    # 可选，~14 MB
```

### 2. 配置

```bash
mkdir -p ~/.opencrab
cp config.example.toml ~/.opencrab/config.toml
```

编辑 `~/.opencrab/config.toml`，最小可用配置：

```toml
[gateway]
host = "127.0.0.1"
port = 18789

[agent]
provider = "openai"                     # 见「支持的 Provider」
model = "gpt-4o"
api_key = "${OPENAI_API_KEY}"           # 支持 ${ENV_VAR} 语法
# base_url = ""                         # 可选：覆盖默认 API 地址
temperature = 0.7
max_tokens = 4096
system_prompt = "You are a helpful assistant."

[logging]
level = "info"
```

设置环境变量：

```bash
export OPENAI_API_KEY="sk-..."
```

### 3. 使用

```bash
# 直接对话（不需要启动 Gateway）
opencrab chat "你好，介绍一下 Rust"

# 启动 Gateway（飞书 Bot + HTTP/WS 服务器）
opencrab gateway run
opencrab gateway run --config ./my-config.toml --port 8080

# 查看通道状态
opencrab channels status

# 查看配置
opencrab config get agent.model
```

---

## 支持的 LLM Provider

基于 [rig-core](https://github.com/0xPlaygrounds/rig)，只需修改 `provider` 和 `model` 即可切换：

| Provider | `provider` 值 | 说明 |
|----------|---------------|------|
| OpenAI | `openai` | GPT-4o 等，Responses API |
| OpenAI 兼容 | `openai-compatible` | 任何兼容端点，需设置 `base_url` |
| Anthropic | `anthropic` | Claude 系列 |
| DeepSeek | `deepseek` | DeepSeek Chat / Reasoner |
| 智谱 AI | `zhipu` | GLM-4 / GLM-4-Flash 等 |
| 智谱 Code | `zhipu-code` | CodeGeeX 代码模型 |
| 智谱海外 | `zhipu-overseas` | Z.AI 海外端点 |
| 智谱海外 Code | `zhipu-overseas-code` | Z.AI 海外代码端点 |
| Ollama | `ollama` | 本地模型，无需 API key |
| Groq | `groq` | 高速推理 |
| Google Gemini | `gemini` | Gemini 系列 |
| Perplexity | `perplexity` | Sonar 搜索增强 |
| Moonshot | `moonshot` | Kimi |
| xAI | `xai` | Grok 系列 |
| OpenRouter | `openrouter` | 多模型路由 |
| Mistral | `mistral` | Mistral Large 等 |
| Together | `together` | 开源模型托管 |
| Cohere | `cohere` | Command R 等 |
| HuggingFace | `huggingface` | HF Inference API |
| Azure OpenAI | `azure` | 需设置 `base_url` + `api_version` |
| Hyperbolic | `hyperbolic` | — |
| Mira | `mira` | — |
| Galadriel | `galadriel` | — |

<details>
<summary><strong>配置示例</strong></summary>

**DeepSeek：**
```toml
[agent]
provider = "deepseek"
model = "deepseek-chat"
api_key = "${DEEPSEEK_API_KEY}"
```

**智谱 AI (GLM)：**
```toml
[agent]
provider = "zhipu"
model = "glm-4-flash"
api_key = "${ZHIPUAI_API_KEY}"
```

**智谱 CodeGeeX：**
```toml
[agent]
provider = "zhipu-code"
model = "codegeex-4"
api_key = "${ZHIPUAI_API_KEY}"
```

**本地 Ollama：**
```toml
[agent]
provider = "ollama"
model = "qwen2.5:14b"
api_key = ""
base_url = "http://localhost:11434"
```

**任意 OpenAI 兼容端点：**
```toml
[agent]
provider = "openai-compatible"
model = "your-model"
api_key = "${API_KEY}"
base_url = "http://your-server:8000/v1"
```

**Azure OpenAI：**
```toml
[agent]
provider = "azure"
model = "gpt-4o"
api_key = "${AZURE_OPENAI_API_KEY}"
base_url = "https://your-resource.openai.azure.com/"
api_version = "2024-10-21"
```

</details>

---

## 飞书 Bot

WebSocket 连接，无需公网 IP，5 分钟完成配置。

### 配置步骤

1. 在 [飞书开放平台](https://open.feishu.cn) 创建企业自建应用
2. 「添加应用能力」→ 开启 **机器人**
3. 「事件与回调」→ 添加 `im.message.receive_v1` 事件 → 选择 **WebSocket** 连接
4. 「凭证与基础信息」→ 记下 `App ID` 和 `App Secret`
5. 「版本管理与发布」→ 创建版本 → 发布

### 填写配置

```toml
[channels.feishu]
enabled = true
app_id = "cli_xxxxxxxxxxxx"
app_secret = "${FEISHU_APP_SECRET}"
domain = "feishu"                   # "feishu" | "lark"
connection_mode = "websocket"
require_mention = true              # 群聊需要 @Bot
```

```bash
export FEISHU_APP_SECRET="your-secret"
```

### 启动

```bash
opencrab gateway run
# OpenCrab Gateway starting...
# Channel feishu: connected
# Listening on 127.0.0.1:18789
```

在飞书中搜索 Bot 名称私聊，或将 Bot 添加到群聊后 @Bot。

---

## Gateway API

启动 Gateway 后可通过 HTTP 和 WebSocket 访问。

**健康检查：**

```bash
curl http://127.0.0.1:18789/health
# {"status":"ok","uptime_secs":42}
```

**WebSocket JSON-RPC 2.0：**

连接 `ws://127.0.0.1:18789/ws`，发送：

```json
{"jsonrpc":"2.0","method":"chat.send","params":{"text":"你好"},"id":1}
```

| 方法 | 参数 | 说明 |
|------|------|------|
| `health` | — | 健康检查 |
| `chat.send` | `{ "text": "..." }` | 发送消息给 Agent |
| `channels.status` | — | 通道状态 |
| `config.get` | `{ "key": "agent.model" }` | 读取配置（留空返回全部） |

---

## 项目结构

```
crates/
├── protocol/    # JSON-RPC 2.0 + 事件定义
├── config/      # TOML 配置加载 + 环境变量替换
├── agent/       # Rig Agent 封装 + 内置工具 (bash, read_file)
├── channels/    # 通道抽象 + 飞书实现
├── gateway/     # Axum HTTP/WS 服务器
└── cli/         # 命令行入口
```

## 技术栈

| 模块 | 依赖 |
|------|------|
| LLM | [rig-core](https://github.com/0xPlaygrounds/rig) — 23 providers |
| HTTP | axum |
| WebSocket | tokio-tungstenite |
| CLI | clap |
| 序列化 | serde + toml + serde_json |
| 异步运行时 | tokio |

## License

MIT
