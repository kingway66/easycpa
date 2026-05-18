# 为什么要自己管理 Claude Code / Codex 的配置文件

## 问题：覆盖式配置管理的风险

cc-switch 等工具的工作方式是**直接覆盖** `~/.claude/settings.json` 和 `~/.codex/config.toml`。

工具每次启动时，它会把整个配置文件重写为新的内容。

虽然采取了通用配置和备份措施，但是逻辑复杂，回退困难，实现不雅。

曾遇到好几次问题，如果不是时间机器能找回，都傻眼了。

这东西就不是这样用的，擦屁股就用手纸，不是用手指。

就算不会用文本编辑器，让AI帮你写配置也可以而且很方便。

> 以下内容AI生成

## 解决方案：参数化配置，而非覆盖

### Claude Code：`--settings` 参数

Claude Code 原生支持 `--settings` 参数，可以指定一个不同的配置文件：

```bash
# 使用 DeepSeek 的配置启动
claude --settings settings.deepseek.json

# 使用 GPT-5.4 的配置启动
claude --settings settings.cpa-gpt.json

# 使用默认配置启动（不受影响）
claude
```

每个 `settings.xxx.json` 文件是独立的，互不干扰。主配置文件 `settings.json` 完全不需要改动。

#### 实际配置示例

`~/.claude/settings.deepseek.json`：
```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "https://api.deepseek.com/anthropic",
    "ANTHROPIC_AUTH_TOKEN": "sk-xxx",
    "ANTHROPIC_MODEL": "deepseek-v4-pro[1m]",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "deepseek-v4-pro[1m]",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "deepseek-v4-pro[1m]",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "deepseek-v4-flash",
    "CLAUDE_CODE_SUBAGENT_MODEL": "deepseek-v4-flash",
    "CLAUDE_CODE_EFFORT_LEVEL": "max"
  }
}
```

`~/.claude/settings.cpa-gpt.json`：
```json
{
  "env": {
    "ANTHROPIC_BASE_URL": "http://127.0.0.1:15721",
    "ANTHROPIC_AUTH_TOKEN": "PROXY_MANAGED",
    "ANTHROPIC_MODEL": "gpt-5.4",
    "ANTHROPIC_DEFAULT_OPUS_MODEL": "gpt-5.4",
    "ANTHROPIC_DEFAULT_SONNET_MODEL": "gpt-5.4",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL": "gpt-5.4",
    "CLAUDE_CODE_SUBAGENT_MODEL": "gpt-5.4"
  }
}
```

可以轻松维护 7+ 个配置文件，每个对应一个模型或提供商，随时切换，互不干扰。

### Codex CLI：`--profile` 参数

Codex CLI 支持 profile 机制，在 `~/.codex/config.toml` 中定义多个 profile：

```toml
[model_providers.ccs-proxy]
name = "ccs-proxy"
base_url = "http://localhost:15721/v1"
wire_api = "responses"
requires_openai_auth = true

[model_providers.rightcode]
name = "rightcode"
base_url = "https://right.codes/codex/v1"
wire_api = "responses"
requires_openai_auth = true

[profiles.ccs-proxy]
preferred_auth_method = "apikey"
model_provider = "ccs-proxy"
model = "gpt-5.5"
model_reasoning_effort = "xhigh"

[profiles.rightcode]
preferred_auth_method = "apikey"
model_provider = "rightcode"
model = "gpt-5.4"
model_reasoning_effort = "xhigh"
```

使用时指定 profile：

```bash
codex --profile ccs-proxy
codex --profile rightcode
```

每个 profile 有自己的 model_provider、model、reasoning_effort，完全独立。

## EasyCPA 的角色

EasyCPA 在这个体系中的定位是：

1. **统一代理** — 一个进程处理所有格式转换（Anthropic ↔ OpenAI Chat ↔ OpenAI Responses）
2. **配置透明** — EasyCPA 自己的 `config.json` 只管路由（哪个模型名转发到哪个上游），不碰客户端的配置文件
3. **客户端自主** — Claude Code 用 `--settings`，Codex 用 `--profile`，各自管理自己的配置

这种分层架构的好处是：

- EasyCPA 的配置变更不影响客户端配置
- 客户端的配置变更不影响代理逻辑
- 每一层可以独立调试、独立回滚

## 最佳实践

1. **永远不要修改 `settings.json` 主文件** — 把 `settings.json` 作为 symlink 指向 `settings.json.base`，切换模型用 `--settings` 参数
2. **每个模型/提供商一个配置文件** — `settings.deepseek.json`、`settings.rightcode.json`、`settings.cpa-gpt.json` 等
3. **Claude 的 settings 文件只放 env 差异** — 通用配置（permissions、statusLine 等）留在主文件，settings 文件只包含 env 字段
4. **Codex 用 profile 隔离** — 每个 provider 对应一个 profile，不要覆盖全局 model 字段
5. **EasyCPA 的 config.json 只管路由** — 不需要在这里配置客户端行为

## 总结

| 做法 | 风险 | 推荐 |
|------|------|------|
| 覆盖 settings.json | 丢失自定义、不可逆 | 用 `--settings` 参数 |
| 覆盖 config.toml | 竞态、不可审计 | 用 `--profile` 参数 |
| 一个配置文件来回改 | 容易出错、无法并行 | 每个模型独立文件 |

配置文件应该像代码一样被版本化管理：独立、可追踪、可回滚。覆盖式管理就像直接在生产环境改代码——能跑，但迟早出问题。
