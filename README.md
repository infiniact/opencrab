# OpenCrab 🦀

Multi-channel AI Gateway — 用 Rust 构建的多通道 AI 网关。

## 快速开始

### 1. 构建

```bash
cargo build --release
strip target/release/opencrab
```

### 2. 配置

```bash
mkdir -p ~/.opencrab
cp config.example.toml ~/.opencrab/config.toml
```

编辑 `~/.opencrab/config.toml`：

```toml
[gateway]
host = "127.0.0.1"
port = 18789

[agent]
provider = "openai"       # "openai" | "anthropic"
model = "gpt-4o"
api_key = "${OPENAI_API_KEY}"   # 支持环境变量
temperature = 0.7
max_tokens = 4096
system_prompt = "You are a helpful assistant."

[channels.feishu]
enabled = true
app_id = "your-app-id"
app_secret = "${FEISHU_APP_SECRET}"
domain = "feishu"              # "feishu" | "lark"
connection_mode = "websocket"
require_mention = true         # 群聊需要 @

[logging]
level = "info"
```

设置环境变量：

```bash
export OPENAI_API_KEY="sk-..."
# 或
export ANTHROPIC_API_KEY="sk-ant-..."

# 飞书
export FEISHU_APP_SECRET="your-secret"
```

### 3. 使用

**直接对话（不需要启动 Gateway）：**

```bash
opencrab chat "你好，介绍一下 Rust"
```

**启动 Gateway（飞书 Bot + HTTP/WS 服务器）：**

```bash
opencrab gateway run
# 或指定配置文件和端口
opencrab gateway run --config ./my-config.toml --port 8080
```

**检查通道状态：**

```bash
opencrab channels status
```

输出：
```
CHANNEL      STATUS       LATENCY
------------------------------------
feishu       connected    -
```

**查看配置：**

```bash
opencrab config get agent.model
# "gpt-4o"

opencrab config get
# 输出完整配置
```

### 4. Gateway API

启动 Gateway 后，可通过 HTTP 和 WebSocket 访问：

**健康检查：**

```bash
curl http://127.0.0.1:18789/health
# {"status":"ok","uptime_secs":42}
```

**WebSocket JSON-RPC：**

连接 `ws://127.0.0.1:18789/ws`，发送 JSON-RPC 2.0 请求：

```json
{"jsonrpc":"2.0","method":"chat.send","params":{"text":"你好"},"id":1}
```

可用方法：

| 方法 | 参数 | 说明 |
|------|------|------|
| `health` | 无 | 健康检查 |
| `chat.send` | `{ "text": "..." }` | 发送消息给 Agent |
| `channels.status` | 无 | 通道状态 |
| `config.get` | `{ "key": "agent.model" }` | 读取配置 |

## 飞书 Bot 配置

1. 在[飞书开放平台](https://open.feishu.cn)创建自建应用
2. 开启 **机器人** 能力
3. 在「事件订阅」中添加 `im.message.receive_v1` 事件
4. 选择 **WebSocket** 连接方式（无需公网 URL）
5. 将 `app_id` 和 `app_secret` 填入配置文件
6. 启动 `opencrab gateway run`
7. 在飞书中与 Bot 私聊，或在群中 @Bot

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

- **LLM**: [rig-core](https://github.com/0xPlaygrounds/rig) (OpenAI + Anthropic)
- **HTTP**: axum
- **WebSocket**: tokio-tungstenite
- **CLI**: clap
- **序列化**: serde + toml + serde_json
- **异步运行时**: tokio

## License

MIT
