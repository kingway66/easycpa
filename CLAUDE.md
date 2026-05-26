# CLAUDE.md — EasyCPA Project Guide

## Project Overview

EasyCPA is a Rust CLI proxy that lets Claude Code and Codex CLI use alternative model providers (DeepSeek, OpenCode, ChatGPT via Codex OAuth, etc.). It translates between API formats (Anthropic Messages ↔ OpenAI Chat ↔ OpenAI Responses) with full streaming SSE support.

## Build & Run

```bash
# Dev run
RUST_LOG=debug cargo run -- serve

# Release build
cargo build --release

# Binary location
./target/release/easycpa
```

## Frontend

The Web UI is a React + Vite + Tailwind app embedded into the binary at compile time via `rust_embed` (reads `frontend/dist/`). **Frontend changes require rebuilding the binary.**

```bash
cd frontend
npm install
npm run build      # outputs to frontend/dist/
cd ..
cargo build --release   # embeds frontend/dist/ into binary
```

## Testing

```bash
cargo test --release
```

Note: some tests may fail due to incomplete features (e.g. Gemini). Use `cargo test --release -- --skip gemini` to skip known-broken tests if needed. `cargo build --release` is the primary gate.

## Release

1. Bump version in `Cargo.toml`.
2. `cd frontend && npm run build && cd ..`.
3. `cargo build --release`.
4. Local install / runtime verification:
   ```bash
   cp target/release/easycpa ~/.local/bin/easycpa
   ~/.local/bin/easycpa restart --address 127.0.0.1 --port 15791
   curl --noproxy '*' -s http://127.0.0.1:15791/status | python3 -m json.tool
   ```
5. **Release means merge to local `main` first, then push from local `main` to `origin/main`** unless the user explicitly asks for a separate branch/PR workflow.
   - Do not leave extra release branches visible on GitHub unless asked.
   - Before pushing, make sure local `main` actually points at the release commit.
6. Commit on local `main`, push `main`, create tag/release:
   ```bash
   git add -A
   git commit -m "fix|feat|chore: ..."
   git push origin main
   git tag -a vX.Y.Z -m "Release vX.Y.Z"
   git push origin vX.Y.Z
   gh release create vX.Y.Z --title "vX.Y.Z" --notes "..."
   ```
7. Package and upload binary zip in the same format as earlier releases:
   ```bash
   mkdir -p /tmp/easycpa-X.Y.Z/easycpa-X.Y.Z
   cp target/release/easycpa /tmp/easycpa-X.Y.Z/easycpa-X.Y.Z/
   cp README.md /tmp/easycpa-X.Y.Z/easycpa-X.Y.Z/
   cp config.json.sample /tmp/easycpa-X.Y.Z/easycpa-X.Y.Z/
   cd /tmp/easycpa-X.Y.Z
   zip -r easycpa-X.Y.Z-macos-arm64.zip easycpa-X.Y.Z
   gh release upload vX.Y.Z easycpa-X.Y.Z-macos-arm64.zip
   ```

The release zip must contain exactly this versioned top-level directory layout:
- `easycpa-X.Y.Z/README.md`
- `easycpa-X.Y.Z/config.json.sample`
- `easycpa-X.Y.Z/easycpa`

## Architecture

### Source Layout

```
src/
├── main.rs              — CLI entry (clap)
├── lib.rs               — Config types (ModelRoute, AppConfig)
├── provider.rs          — Provider model + ProviderMeta
├── config.rs            — Config file loading
├── store.rs             — App state store
├── error.rs             — Top-level error types
├── provider_defaults.rs — Preset provider templates
├── database/            — rusqlite persistence (providers, failover, usage, settings)
└── proxy/               — Core proxy engine
    ├── server.rs        — Axum router setup
    ├── handlers.rs      — Request handlers
    ├── handler_context.rs — Route matching → Provider resolution
    ├── forwarder.rs     — Main request forwarding logic
    ├── http_client.rs   — reqwest client builder (global + per-route proxy)
    ├── hyper_client.rs  — Low-level hyper client for raw streaming
    ├── providers/       — Provider adapters + format transformers
    │   ├── adapter.rs   — ProviderAdapter trait
    │   ├── auth.rs      — AuthInfo + AuthStrategy enum
    │   ├── codex.rs     — OpenAI/Codex adapter
    │   ├── codex_oauth_auth.rs — Read ~/.codex/auth.json for Codex OAuth
    │   ├── claude.rs    — Anthropic adapter
    │   ├── transform.rs — Anthropic ↔ OpenAI Chat transform
    │   ├── transform_responses.rs     — Anthropic ↔ OpenAI Responses
    │   ├── transform_responses_chat.rs — OpenAI Responses ↔ Chat
    │   ├── streaming*.rs — SSE stream transformers
    │   └── models/      — Request/response type definitions
    ├── circuit_breaker.rs   — Three-state circuit breaker
    ├── failover_switch.rs   — Auto failover logic
    ├── thinking_optimizer.rs     — Thinking strategy selector
    ├── thinking_rectifier.rs    — Signature fix + retry
    ├── thinking_budget_rectifier.rs — Budget constraint fix + retry
    ├── cache_injector.rs   — Prompt caching breakpoint injection
    ├── sse.rs              — SSE parsing utilities
    ├── static_files.rs     — Embedded SPA serving (rust_embed)
    └── usage/              — Usage tracking and logging
```

### Key Concepts

- **ModelRoute**: user-facing config mapping (name → model, base_url, api_key, api_format, proxy_url). `name: "*"` is wildcard.
- **Provider**: internal representation derived from ModelRoute or database, includes `ProviderMeta` (api_format, proxy_url).
- **api_format**: `openai_chat` (default), `openai_responses`, `anthropic`.
- **AuthStrategy**: Bearer, Anthropic, ClaudeAuth, Google, GoogleOAuth, GitHubCopilot, CodexOAuth.
- **Per-route proxy**: each route can have its own `proxy_url` (HTTP/SOCKS5). No system env proxy is used.
- **Codex OAuth**: `api_key: "read_codex_auth"` triggers reading `~/.codex/auth.json` for access token + account info.

### Frontend

```
frontend/src/
├── components/    — Layout, shared UI
├── context/       — ApiContext (fetch/state)
├── pages/         — ModelDetail (route CRUD), Settings, Providers
└── App.tsx        — React Router setup
```

Config path: `~/.easycpa/config.json` or `./config.json` (alongside binary).

## Code Style

- Rust edition 2021, MSRV 1.85
- Chinese comments are fine (existing codebase uses them)
- No unnecessary comments — only for non-obvious WHY
- Prefer editing existing files over creating new ones
- Frontend: React 19 + TypeScript + Tailwind CSS v4 + Vite

## Known Issues

- `cargo test` has pre-existing Gemini-related compilation errors unrelated to most changes
- Frontend is embedded at compile time — always `npm run build` before `cargo build --release`

## Debug & Troubleshooting

### 日志位置
`~/.easycpa/logs/easycpa.log`

### 开启 debug 日志
```bash
# 全局 debug
pkill easycpa; RUST_LOG=debug ~/.local/bin/easycpa start --foreground --no-open

# 只看 reqwest 连接细节
RUST_LOG=info,reqwest::connect=debug ~/.local/bin/easycpa start --foreground --no-open

# 后台启动（debug 写入日志文件）
RUST_LOG=debug ~/.local/bin/easycpa start
```

### 常用调试命令
```bash
# 查看最新错误
grep -i 'error\|WARN.*FWD' ~/.easycpa/logs/easycpa.log | tail -20

# 查看特定模型的请求
grep 'gpt-5.4' ~/.easycpa/logs/easycpa.log | tail -20

# 检查 /status（绕过系统代理）
curl --noproxy '*' -s http://127.0.0.1:15791/status | python3 -m json.tool

# 测试代理连通性
curl -x http://127.0.0.1:10910 --connect-timeout 10 -s -o /dev/null -w "%{http_code}" https://chatgpt.com/
```

### 注意事项
- curl 默认走系统代理（`ALL_PROXY`），测试本地服务时加 `--noproxy '*'`
- reqwest 使用 native-tls（非 rustls-tls），因为 rustls 通过 HTTP 代理 CONNECT 隧道连 chatgpt.com 不稳定
- CodexOAuth 路径（chatgpt.com）的请求体使用白名单过滤，只保留 codex-rs `ResponsesApiRequest` 结构体里的字段
