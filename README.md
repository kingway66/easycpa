# EasyCPA

CPA-compatible proxy for Claude Code, Codex & Gemini CLI.

本项目解决三个问题：

1. **传统 OpenAI Chat 端点在 Codex / Claude Code 中使用** — 如 OpenCode Go 等只提供 `openai_chat` 格式的端点，无法直接接入 Codex（Responses API）或 Claude Code（Anthropic Messages API）。EasyCPA 自动完成格式转换与流式 SSE 适配。
2. **GPT-5.4 等 OpenAI Responses 端点在 Claude Code 中使用** — Responses API 格式的上游无法直接对接 Claude Code 的 Anthropic Messages 协议，EasyCPA 提供双向转换。
3. **DeepSeek reasoning_content / image 补丁** — DeepSeek 返回的 `reasoning_content` 字段和 base64 图片在标准 OpenAI Chat 协议中无对应，EasyCPA 自动将其转换为客户端可识别的格式，并过滤不支持图片的上游以避免报错。

代码整合了 [cc-switch](https://github.com/farion1231/cc-switch)、[CPA](https://github.com/samueltuyizere/oc-go-cc) 等项目的思路，用 Claude Code + GLM-5.1 改写。

## 核心能力

### 多格式自动转换

| 客户端 | 请求格式 | 上游格式 | 转换模块 |
|--------|---------|---------|---------|
| Claude Code | Anthropic Messages | OpenAI Chat | `transform.rs` |
| Codex CLI | OpenAI Responses | OpenAI Chat | `transform_responses_chat.rs` |
| Claude Code | Anthropic Messages | OpenAI Responses | `transform_responses.rs` |
| Gemini CLI | Anthropic Messages | Gemini Native | `transform_gemini.rs` |

所有转换均为双向：请求方向自动转换格式，响应方向（含 SSE 流式）同步转换回客户端期望的格式。

### 流式 SSE 转换

- Chat Completions SSE → Anthropic Messages SSE（Claude Code 路径）
- Chat Completions SSE → OpenAI Responses SSE（Codex 路径）
- Gemini SSE → Anthropic Messages SSE（Gemini 路径）
- 流式首字超时 / 静默超时 / 非流式总超时，三重超时保护

### 故障转移 & 熔断器

- 自动故障转移：请求失败时尝试下一个可用 provider
- 三态熔断器（Closed → Open → HalfOpen）：连续失败自动熔断，恢复期放行试探请求
- 故障转移去重：多个并发请求不会重复触发切换
- 每个应用独立熔断状态，互不干扰

### 429 退避重试

- 单 Provider 场景下 429 不直接返回给客户端
- 自动退避重试（2s → 4s → 8s，最多 3 次）
- 避免客户端（如 Codex）立即重试导致恶性循环

### Copilot 请求优化器

解决 GitHub Copilot 代理消耗量异常（Issue #1813），包含 7 个子功能：

1. **请求分类** — 根据消息结构自动判断 `x-initiator: user/agent`，agent 续写不计 premium interaction
2. **Tool result 合并** — 多个 tool_result 合并为一条消息，减少请求次数
3. **Compact 检测** — 识别上下文压缩请求，标记为 agent 类型
4. **确定性 Request ID** — 相同输入生成相同 ID，避免重复计费
5. **Subagent 检测** — 识别 Claude Code 子代理请求，不计 premium quota
6. **Warmup 降级** — 探针请求自动降级到小模型（如 gpt-5-mini），避免浪费 premium quota
7. **Strip thinking** — 请求前主动剥离 assistant 消息中的 thinking/redacted_thinking block，避免上游拒绝触发重复扣费

### Thinking 优化 & 整流

- **Thinking 优化器** — 根据模型自动选择最佳 thinking 策略：haiku 跳过、opus/sonnet 使用 adaptive thinking、其他模型注入 budget_tokens
- **Thinking 签名整流器** — 上游返回签名校验错误时，自动移除问题 signature 字段并重试
- **Thinking Budget 整流器** — 上游返回 budget_tokens 约束错误时，自动调整参数并重试
- **Cache 断点注入** — 自动在 tools/system/user 关键位置注入 `cache_control` 断点，启用 Bedrock Prompt Caching

### 图片处理

- Codex vision-ability 发送的 base64 图片，自动保存为本地文件（`/tmp/easycpa-images/`）
- 使用内容 SHA256 hash 作为文件名，同一图片始终映射到同一文件，不重复保存
- 不支持图片的上游（如 DeepSeek）不会收到 `image_url` 类型的 `unknown variant` 错误
- 支持图片的上游可通过文件路径读取图片内容

### Reasoning Effort 映射

- DeepSeek 模型自动映射 reasoning_effort：`xhigh → max`，`low/medium → high`
- Claude Code 路径和 Codex 路径均已支持
- 对没有 reasoning_content 问题的端点无副作用

### /v1/models 端点

- 返回 Codex ModelInfo 格式（`{ models: [ModelInfo] }`）
- 支持 `supported_reasoning_levels`、`supports_reasoning_summaries`、`supports_parallel_tool_calls`、`context_window`、`max_output_tokens` 等完整字段
- config 中配置的能力字段优先，未配置则使用合理 fallback 默认值
- `supports_reasoning_summaries = true` 是 Codex 发送 reasoning_effort 的门控开关

## 支持的客户端

| 客户端 | 接入方式 |
|--------|---------|
| Claude Code | Anthropic Messages API (`/v1/messages`) |
| Claude Desktop | Anthropic Messages API（带 gateway 认证） |
| Codex CLI | OpenAI Responses API (`/v1/responses`) |
| Gemini CLI | Gemini Native API (`/v1beta/*`) |
| GitHub Copilot | Anthropic Messages API（带 Copilot 优化） |
| OpenCode | OpenAI Chat/Responses API |
| OpenClaw | OpenAI Chat API |

## 配置

配置文件路径：`~/.easycpa/config.json`

```json
{
  "models": [
    {
      "name": "deepseek-v4-flash-free",
      "model": "deepseek-v4-flash-free",
      "base_url": "https://opencode.ai/zen/v1",
      "api_key": "sk-xxx",
      "api_format": "openai_chat",
      "context_window": 1000000,
      "max_output_tokens": 384000,
      "default_reasoning_level": "high",
      "supported_reasoning_levels": ["low", "medium", "high", "xhigh"],
      "supports_parallel_tool_calls": true,
      "supports_reasoning_summaries": true
    },
    {
      "name": "gpt-5.5",
      "model": "deepseek-v4-pro",
      "base_url": "https://opencode.ai/zen/go/v1",
      "api_key": "sk-xxx",
      "api_format": "openai_chat",
      "context_window": 1000000,
      "max_output_tokens": 384000,
      "default_reasoning_level": "high",
      "supported_reasoning_levels": ["low", "medium", "high", "xhigh"],
      "supports_parallel_tool_calls": true,
      "supports_reasoning_summaries": true
    },
    {
      "name": "gpt-5.4",
      "model": "gpt-5.4",
      "base_url": "https://right.codes/codex",
      "api_key": "sk-xxx",
      "api_format": "openai_responses"
    },
    {
      "name": "*",
      "model": "*",
      "base_url": "https://localhost/v1",
      "api_key": "sk-xxx",
      "api_format": "openai_chat"
    }
  ],
  "listen": "127.0.0.1:15721"
}
```

### ModelRoute 字段说明

| 字段 | 必填 | 说明 |
|------|------|------|
| `name` | 否 | 路由名称，客户端请求的 model 字段匹配此项，默认等于 model |
| `model` | 是 | 实际转发给上游的模型名 |
| `base_url` | 是 | 上游 API 地址 |
| `api_key` | 是 | API 密钥 |
| `api_format` | 否 | API 格式：`openai_chat`（默认）/ `openai_responses` / `anthropic` / `gemini_native` |
| `context_window` | 否 | 上下文窗口大小，用于 /v1/models 返回 |
| `max_output_tokens` | 否 | 最大输出 token 数 |
| `default_reasoning_level` | 否 | 默认推理级别 |
| `supported_reasoning_levels` | 否 | 支持的推理级别列表 |
| `supports_parallel_tool_calls` | 否 | 是否支持并行工具调用 |
| `supports_reasoning_summaries` | 否 | 是否支持推理摘要（Codex 门控开关） |

`name: "*"` 为通配路由，匹配所有未命中的模型请求。

## API 端点

```
GET  /health                    — 健康检查
GET  /status                    — 代理状态
GET  /v1/models                 — 模型列表（Codex ModelInfo 格式）
POST /v1/messages               — Anthropic Messages API
POST /claude/v1/messages        — Claude Desktop Messages API
POST /v1/chat/completions       — OpenAI Chat Completions API
POST /chat/completions          — OpenAI Chat Completions API
POST /v1/responses              — OpenAI Responses API
POST /codex/v1/responses        — Codex Responses API
POST /v1beta/*path              — Gemini Native API
```

## 编译 & 运行

```bash
cargo build --release
./target/release/easycpa serve
```

开发模式：

```bash
RUST_LOG=debug cargo run -- serve
```

## 技术栈

- **语言**: Rust (edition 2021, MSRV 1.85)
- **HTTP 框架**: axum + hyper
- **异步运行时**: tokio
- **序列化**: serde + serde_json
- **CLI**: clap
- **数据库**: rusqlite（provider 配置、usage 统计、熔断器状态持久化）
- **加密/哈希**: sha2

## 项目结构

```
src/
├── main.rs                      # CLI 入口 (clap)
├── lib.rs                       # 核心逻辑：配置加载、provider 路由
├── proxy/
│   ├── server.rs                # axum HTTP 服务器 & 路由注册
│   ├── handlers.rs              # 请求处理器
│   ├── circuit_breaker.rs       # 三态熔断器
│   ├── failover_switch.rs       # 故障转移切换管理
│   ├── forwarder.rs             # 请求转发（含 429 退避重试）
│   ├── health.rs                # 健康检查
│   ├── copilot_optimizer.rs     # Copilot 请求优化器
│   ├── thinking_optimizer.rs    # Thinking 优化器
│   ├── thinking_rectifier.rs    # Thinking 签名整流器
│   ├── thinking_budget_rectifier.rs  # Thinking Budget 整流器
│   ├── cache_injector.rs        # Bedrock Cache 断点注入
│   ├── body_filter.rs           # 请求体过滤
│   ├── error_mapper.rs          # 错误映射
│   ├── sse.rs                   # SSE 解析工具
│   ├── types.rs                 # 类型定义
│   ├── usage/                   # Usage 统计 & 计费
│   └── providers/
│       ├── transform.rs                # Anthropic ↔ OpenAI Chat 转换
│       ├── transform_responses_chat.rs  # OpenAI Responses → OpenAI Chat 转换
│       ├── transform_responses.rs       # Anthropic ↔ OpenAI Responses 转换
│       ├── transform_gemini.rs          # Anthropic ↔ Gemini 转换
│       ├── streaming.rs                 # Anthropic SSE 流式处理
│       ├── streaming_responses.rs       # OpenAI Responses SSE 流式处理
│       ├── streaming_chat_to_responses.rs  # Chat SSE → Responses SSE
│       ├── streaming_gemini.rs          # Gemini SSE 流式处理
│       ├── claude.rs / codex.rs / gemini.rs  # Provider 适配器
│       └── models/                      # OpenAI/Anthropic 数据模型
├── database/                    # SQLite 数据层
├── services/                    # 业务服务
│   ├── proxy.rs                 # 代理服务
│   ├── stream_check.rs          # 流式检查
│   ├── subscription.rs          # 订阅管理
│   └── usage_cache.rs           # Usage 缓存
└── claude_desktop_config.rs     # Claude Desktop 配置接管
```

## 代码来源

本项目从 [cc-switch](https://github.com/farion1231/cc-switch)（MIT License, © Jason Young）提取并独立演化。

核心代理逻辑（格式转换、流式 SSE、熔断器、Copilot 优化、Thinking 优化/整流等）源自 cc-switch 的 `src-tauri/src/proxy/` 模块，重构为独立 CLI 工具时移除了 Tauri 桌面端依赖，并新增了以下功能：

- Codex Responses → OpenAI Chat 格式转换（`transform_responses_chat.rs`、`streaming_chat_to_responses.rs`）
- Chat SSE → Responses SSE 流式转换
- Reasoning 内容处理（DeepSeek reasoning_content → Responses reasoning item）
- 多 function_call 合并到同一 assistant 消息
- 连续 assistant 消息合并修复
- 429 退避重试
- 独立 CLI 入口（clap）

## License

MIT
