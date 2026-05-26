//! OpenAI Responses API ↔ OpenAI Chat Completions API 格式转换
//!
//! 当 Codex 客户端发送 Responses 格式请求，但上游 provider 只支持 Chat 格式时使用。
//! 参考 CPA (CLIProxyAPI) 的 Go 实现。

use crate::proxy::error::ProxyError;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::{json, Value};
use std::path::Path;

fn is_deepseek_model(model: &str) -> bool {
    model.starts_with("deepseek-")
}

/// 保存 base64 图片到本地文件，返回文件路径
/// 使用内容 SHA256 hash 作为文件名，同一图片始终映射到同一文件，不会重复保存
fn save_base64_image(data_url: &str) -> Option<String> {
    if !data_url.starts_with("data:") {
        return None;
    }
    let rest = &data_url[5..];
    let semi = rest.find(';')?;
    let mime = &rest[..semi];
    let b64_data = rest.get(semi + 1..)?.strip_prefix("base64,")?;

    let ext = match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "png",
    };

    let decoded = BASE64.decode(b64_data).ok()?;

    // 用内容 hash 作为文件名，同图同名
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&decoded);
    let hash = hasher.finalize();
    let hex = format!("{:x}", hash);
    // 取前 16 字符作为短文件名，足以避免碰撞
    let short_hash = &hex[..16];

    let dir = Path::new("/tmp/easycpa-images");
    let _ = std::fs::create_dir_all(dir);

    let filename = format!("{}.{}", short_hash, ext);
    let path = dir.join(&filename);
    // 文件已存在则直接返回路径，不重复写入
    if !path.exists() {
        std::fs::write(&path, &decoded).ok()?;
    }

    Some(path.to_string_lossy().to_string())
}

/// OpenAI Responses 请求 → OpenAI Chat Completions 请求
pub fn responses_to_chat_request(body: Value) -> Result<Value, ProxyError> {
    let mut result = json!({});

    if let Some(model) = body.get("model").and_then(|m| m.as_str()) {
        result["model"] = json!(model);
    }

    // instructions → system message
    if let Some(instructions) = body.get("instructions").and_then(|i| i.as_str()) {
        if !instructions.is_empty() {
            result["messages"] = json!([{"role": "system", "content": instructions}]);
        }
    }

    // input → messages
    if let Some(input) = body.get("input").and_then(|i| i.as_array()) {
        let messages = convert_input_to_messages(input)?;
        if let Some(msgs) = result.get_mut("messages") {
            if let Some(arr) = msgs.as_array_mut() {
                arr.extend(messages);
            }
        } else {
            result["messages"] = json!(messages);
        }
    }

    // max_output_tokens → max_tokens
    if let Some(v) = body.get("max_output_tokens") {
        result["max_tokens"] = v.clone();
    }

    // 直接透传
    for field in &["temperature", "top_p", "stream", "store"] {
        if let Some(v) = body.get(*field) {
            result[*field] = v.clone();
        }
    }

    // reasoning.effort → reasoning_effort
    // DeepSeek: xhigh → max, low/medium → high
    if let Some(effort) = body.pointer("/reasoning/effort").and_then(|v| v.as_str()) {
        let mapped = if is_deepseek_model(body.get("model").and_then(|m| m.as_str()).unwrap_or(""))
        {
            match effort {
                "xhigh" => "max",
                "low" | "medium" => "high",
                other => other,
            }
        } else {
            effort
        };
        result["reasoning_effort"] = json!(mapped);
    }

    // tools: Responses function → Chat function
    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        let chat_tools: Vec<Value> = tools
            .iter()
            .filter(|t| t.get("type").and_then(|v| v.as_str()) == Some("function"))
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.get("name").and_then(|n| n.as_str()).unwrap_or(""),
                        "description": t.get("description"),
                        "parameters": t.get("parameters").cloned().unwrap_or(json!({}))
                    }
                })
            })
            .collect();
        if !chat_tools.is_empty() {
            result["tools"] = json!(chat_tools);
        }
    }

    // tool_choice
    if let Some(tc) = body.get("tool_choice") {
        result["tool_choice"] = map_tool_choice_to_chat(tc);
    }

    Ok(result)
}

/// OpenAI Chat Completions 响应 → OpenAI Responses 响应
pub fn chat_to_responses_response(body: Value) -> Result<Value, ProxyError> {
    let choices = body
        .get("choices")
        .and_then(|c| c.as_array())
        .ok_or_else(|| ProxyError::TransformError("No choices in Chat response".to_string()))?;

    let choice = choices
        .first()
        .ok_or_else(|| ProxyError::TransformError("Empty choices array".to_string()))?;

    let message = choice
        .get("message")
        .ok_or_else(|| ProxyError::TransformError("No message in choice".to_string()))?;

    let mut output_items = Vec::new();

    // 构建 message item
    let mut content_parts = Vec::new();

    // reasoning_content → reasoning item
    let reasoning_text = message
        .get("reasoning_content")
        .and_then(|r| r.as_str())
        .filter(|r| !r.is_empty());

    if let Some(reasoning) = reasoning_text {
        output_items.push(json!({
            "type": "reasoning",
            "id": "",
            "summary": [{"type": "summary_text", "text": reasoning}]
        }));
    }

    // 文本内容
    if let Some(msg_content) = message.get("content") {
        if let Some(text) = msg_content.as_str() {
            if !text.is_empty() {
                content_parts.push(json!({"type": "output_text", "text": text}));
            }
        } else if let Some(parts) = msg_content.as_array() {
            for part in parts {
                let part_type = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match part_type {
                    "text" | "output_text" => {
                        if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                            if !text.is_empty() {
                                content_parts.push(json!({"type": "output_text", "text": text}));
                            }
                        }
                    }
                    "refusal" => {
                        if let Some(refusal) = part.get("refusal").and_then(|r| r.as_str()) {
                            if !refusal.is_empty() {
                                content_parts.push(json!({"type": "refusal", "refusal": refusal}));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    if let Some(refusal) = message.get("refusal").and_then(|r| r.as_str()) {
        if !refusal.is_empty() {
            content_parts.push(json!({"type": "refusal", "refusal": refusal}));
        }
    }

    // Emit message item if there's text content.
    // If there's no text but has tool_calls, skip message item — Codex handles
    // function_call items directly. Injecting a reasoning-as-message item would
    // corrupt the next request's messages (DeepSeek expects tool_result after tool_calls).
    let has_tool_calls = message
        .get("tool_calls")
        .and_then(|t| t.as_array())
        .is_some_and(|a| !a.is_empty());

    if content_parts.is_empty() && !has_tool_calls {
        // No text and no tool calls — use reasoning as fallback content
        // so Codex gets a displayable message
        if let Some(reasoning) = reasoning_text {
            content_parts.push(json!({"type": "output_text", "text": reasoning}));
        }
    }

    if !content_parts.is_empty() {
        output_items.push(json!({
            "type": "message",
            "role": "assistant",
            "content": content_parts
        }));
    }

    // tool_calls → function_call items
    let empty_func = json!({});
    if let Some(tool_calls) = message.get("tool_calls").and_then(|t| t.as_array()) {
        for tc in tool_calls {
            let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");
            let func = tc.get("function").unwrap_or(&empty_func);
            let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = func
                .get("arguments")
                .and_then(|a| a.as_str())
                .unwrap_or("{}");
            output_items.push(json!({
                "type": "function_call",
                "call_id": id,
                "name": name,
                "arguments": args
            }));
        }
    }

    // finish_reason → status
    let finish_reason = choice.get("finish_reason").and_then(|r| r.as_str());
    let has_tool_use = message
        .get("tool_calls")
        .and_then(|t| t.as_array())
        .is_some_and(|a| !a.is_empty());
    let status = match finish_reason {
        Some("stop") => "completed",
        Some("length") => "incomplete",
        Some("tool_calls") | Some("function_call") => "completed",
        _ => "completed",
    };
    let incomplete_details = if finish_reason == Some("length") {
        Some(json!({"reason": "max_output_tokens"}))
    } else {
        None
    };

    // usage
    let usage = body.get("usage").cloned().unwrap_or(json!({}));
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let mut usage_resp = json!({
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "total_tokens": input_tokens + output_tokens
    });
    if let Some(cached) = usage
        .pointer("/prompt_tokens_details/cached_tokens")
        .and_then(|v| v.as_u64())
    {
        usage_resp["input_tokens_details"] = json!({"cached_tokens": cached});
    }

    let mut result = json!({
        "id": body.get("id").and_then(|i| i.as_str()).unwrap_or(""),
        "object": "response",
        "status": status,
        "model": body.get("model").and_then(|m| m.as_str()).unwrap_or(""),
        "output": output_items,
        "usage": usage_resp
    });

    if let Some(details) = incomplete_details {
        result["incomplete_details"] = details;
    }

    if has_tool_use && status == "completed" {
        result["status"] = json!("completed");
    }

    Ok(result)
}

/// Responses input → Chat messages
fn convert_input_to_messages(input: &[Value]) -> Result<Vec<Value>, ProxyError> {
    let mut messages = Vec::new();
    let mut pending_reasoning: Option<String> = None;

    for item in input {
        let item_type = item
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("message");

        match item_type {
            "reasoning" => {
                // Collect reasoning content to merge into next assistant message
                if let Some(summary) = item.get("summary").and_then(|s| s.as_array()) {
                    let text: String = summary
                        .iter()
                        .filter_map(|s| {
                            if s.get("type").and_then(|t| t.as_str()) == Some("summary_text") {
                                s.get("text").and_then(|t| t.as_str()).map(String::from)
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("");
                    if !text.is_empty() {
                        pending_reasoning = Some(text);
                    }
                }
            }

            "message" => {
                let role = match item.get("role").and_then(|r| r.as_str()).unwrap_or("user") {
                    "developer" => "system",
                    other => other,
                };
                if role == "assistant" {
                    // Assistant message — may need reasoning_content
                    let reasoning = pending_reasoning.take();
                    if let Some(content) = item.get("content") {
                        if let Some(text) = content.as_str() {
                            let mut msg = json!({"role": role, "content": text});
                            if let Some(r) = &reasoning {
                                msg["reasoning_content"] = json!(r);
                            }
                            messages.push(msg);
                        } else if let Some(parts) = content.as_array() {
                            let mut content_parts = Vec::new();
                            for part in parts {
                                let part_type =
                                    part.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                match part_type {
                                    "input_text" | "output_text" => {
                                        if let Some(text) =
                                            part.get("text").and_then(|t| t.as_str())
                                        {
                                            content_parts
                                                .push(json!({"type": "text", "text": text}));
                                        }
                                    }
                                    "input_image" => {
                                        if let Some(url) =
                                            part.get("image_url").and_then(|u| u.as_str())
                                        {
                                            if let Some(local_path) = save_base64_image(url) {
                                                content_parts.push(json!({
                                                    "type": "text",
                                                    "text": format!("[Image: {}]", local_path)
                                                }));
                                            } else {
                                                content_parts.push(json!({
                                                    "type": "text",
                                                    "text": "[Image: decode failed]"
                                                }));
                                            }
                                        } else {
                                            content_parts.push(json!({
                                                "type": "text",
                                                "text": "[Image: no data]"
                                            }));
                                        }
                                    }
                                    "refusal" => {
                                        if let Some(refusal) =
                                            part.get("refusal").and_then(|r| r.as_str())
                                        {
                                            content_parts
                                                .push(json!({"type": "text", "text": refusal}));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if content_parts.len() == 1 {
                                if let Some(text) = content_parts[0].get("text") {
                                    let mut msg = json!({"role": role, "content": text});
                                    if let Some(r) = &reasoning {
                                        msg["reasoning_content"] = json!(r);
                                    }
                                    messages.push(msg);
                                } else {
                                    let mut msg = json!({"role": role, "content": content_parts});
                                    if let Some(r) = &reasoning {
                                        msg["reasoning_content"] = json!(r);
                                    }
                                    messages.push(msg);
                                }
                            } else if !content_parts.is_empty() {
                                let mut msg = json!({"role": role, "content": content_parts});
                                if let Some(r) = &reasoning {
                                    msg["reasoning_content"] = json!(r);
                                }
                                messages.push(msg);
                            } else if let Some(r) = reasoning {
                                messages.push(
                                    json!({"role": role, "content": null, "reasoning_content": r}),
                                );
                            }
                        }
                    } else if let Some(r) = reasoning {
                        messages
                            .push(json!({"role": role, "content": null, "reasoning_content": r}));
                    }
                } else {
                    // Non-assistant message — discard pending reasoning
                    pending_reasoning = None;
                    if let Some(content) = item.get("content") {
                        if let Some(text) = content.as_str() {
                            messages.push(json!({"role": role, "content": text}));
                        } else if let Some(parts) = content.as_array() {
                            let mut content_parts = Vec::new();
                            for part in parts {
                                let part_type =
                                    part.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                match part_type {
                                    "input_text" | "output_text" => {
                                        if let Some(text) =
                                            part.get("text").and_then(|t| t.as_str())
                                        {
                                            content_parts
                                                .push(json!({"type": "text", "text": text}));
                                        }
                                    }
                                    "input_image" => {
                                        if let Some(url) =
                                            part.get("image_url").and_then(|u| u.as_str())
                                        {
                                            if let Some(local_path) = save_base64_image(url) {
                                                content_parts.push(json!({
                                                    "type": "text",
                                                    "text": format!("[Image: {}]", local_path)
                                                }));
                                            } else {
                                                content_parts.push(json!({
                                                    "type": "text",
                                                    "text": "[Image: decode failed]"
                                                }));
                                            }
                                        } else {
                                            content_parts.push(json!({
                                                "type": "text",
                                                "text": "[Image: no data]"
                                            }));
                                        }
                                    }
                                    "refusal" => {
                                        if let Some(refusal) =
                                            part.get("refusal").and_then(|r| r.as_str())
                                        {
                                            content_parts
                                                .push(json!({"type": "text", "text": refusal}));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if content_parts.len() == 1 {
                                if let Some(text) = content_parts[0].get("text") {
                                    messages.push(json!({"role": role, "content": text}));
                                } else {
                                    messages.push(json!({"role": role, "content": content_parts}));
                                }
                            } else if !content_parts.is_empty() {
                                messages.push(json!({"role": role, "content": content_parts}));
                            }
                        }
                    }
                }
            }

            "function_call" => {
                let call_id = item.get("call_id").and_then(|i| i.as_str()).unwrap_or("");
                let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let arguments = item
                    .get("arguments")
                    .and_then(|a| a.as_str())
                    .unwrap_or("{}");
                let reasoning = pending_reasoning.take();

                // Multiple function_calls should be merged into the same assistant message's
                // tool_calls array (OpenAI Chat format). If the previous message is also an
                // assistant with tool_calls, append to it instead of creating a new one.
                let tc_entry = json!({
                    "id": call_id,
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": arguments
                    }
                });

                let mut merged = false;
                if let Some(last) = messages.last_mut() {
                    if last.get("role").and_then(|r| r.as_str()) == Some("assistant")
                        && last.get("tool_calls").is_some()
                    {
                        if let Some(tool_calls) =
                            last.get_mut("tool_calls").and_then(|tc| tc.as_array_mut())
                        {
                            tool_calls.push(tc_entry.clone());
                            merged = true;
                        }
                    }
                }

                if !merged {
                    let mut msg = json!({
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [tc_entry]
                    });
                    // DeepSeek/Kimi: reasoning_content 必须和 tool_calls 一起回传
                    let reasoning_content = match reasoning {
                        Some(r) if !r.is_empty() => r,
                        _ => "tool call".to_string(),
                    };
                    msg["reasoning_content"] = json!(reasoning_content);
                    messages.push(msg);
                }
            }

            "function_call_output" => {
                // Discard any pending reasoning before tool output
                pending_reasoning = None;
                let call_id = item.get("call_id").and_then(|i| i.as_str()).unwrap_or("");
                let output = item.get("output").and_then(|o| o.as_str()).unwrap_or("");
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": output
                }));
            }

            _ => {}
        }
    }

    // 修复 tool_calls 序列：每个 assistant 消息的 tool_calls 必须紧随对应的 tool 消息
    fix_tool_calls_sequence(&mut messages);

    Ok(messages)
}

/// 修复 Chat messages 中 tool_calls 的序列问题
///
/// 1. 合并连续的 assistant 消息（Codex 会话历史中可能出现 reasoning-as-message
///    导致的连续 assistant 消息，DeepSeek 不允许这种情况）
/// 2. DeepSeek 等上游要求：assistant 消息带 tool_calls 后，必须紧跟对应的 tool 消息。
///    如果 function_call_output 缺失或顺序不对，需要补占位 tool 消息。
fn fix_tool_calls_sequence(messages: &mut Vec<Value>) {
    // Phase 1: 合并连续的 assistant 消息
    // 例如：assistant(text) + assistant(tool_calls) → assistant(text + tool_calls)
    loop {
        let mut merged = false;
        let mut i = 0;
        while i + 1 < messages.len() {
            let is_assistant = |idx: usize| {
                messages[idx].get("role").and_then(|r| r.as_str()) == Some("assistant")
            };
            if is_assistant(i) && is_assistant(i + 1) {
                // Merge messages[i+1] into messages[i]
                let next = messages.remove(i + 1);

                // Merge content
                let curr_content = messages[i].get("content").cloned();
                let next_content = next.get("content").cloned();

                // If current has no tool_calls and next has tool_calls, merge them
                let curr_has_tc = messages[i].get("tool_calls").is_some();
                let next_has_tc = next.get("tool_calls").is_some();

                if !curr_has_tc && next_has_tc {
                    // Move tool_calls from next to current
                    if let Some(tc) = next.get("tool_calls").cloned() {
                        messages[i]["tool_calls"] = tc;
                    }
                }

                // Merge content: keep non-empty content
                if let Some(nc) = next_content {
                    if nc.as_str().is_some_and(|s| !s.is_empty()) {
                        let cc_empty = curr_content
                            .as_ref()
                            .is_none_or(|c| c.as_str().is_some_and(|s| s.is_empty()));
                        if cc_empty {
                            messages[i]["content"] = nc;
                        }
                    }
                }

                // Merge reasoning_content
                if let Some(rc) = next.get("reasoning_content").cloned() {
                    if messages[i].get("reasoning_content").is_none() {
                        messages[i]["reasoning_content"] = rc;
                    }
                }

                log::debug!("[Transform] 合并连续 assistant 消息: idx={}", i);
                merged = true;
            }
            i += 1;
        }
        if !merged {
            break;
        }
    }

    // Phase 2: 补缺失的 tool 消息
    let mut i = 0;
    while i < messages.len() {
        let msg = &messages[i];
        if msg.get("role").and_then(|r| r.as_str()) == Some("assistant") {
            if let Some(tool_calls) = msg.get("tool_calls").and_then(|t| t.as_array()) {
                if tool_calls.is_empty() {
                    i += 1;
                    continue;
                }

                // 收集需要的 tool_call_id
                let required_ids: Vec<String> = tool_calls
                    .iter()
                    .filter_map(|tc| tc.get("id").and_then(|id| id.as_str()).map(String::from))
                    .collect();

                // 检查后续消息是否已提供对应的 tool 消息
                let mut provided_ids: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                let mut j = i + 1;
                while j < messages.len() {
                    let next = &messages[j];
                    if next.get("role").and_then(|r| r.as_str()) == Some("tool") {
                        if let Some(tid) = next.get("tool_call_id").and_then(|t| t.as_str()) {
                            provided_ids.insert(tid.to_string());
                        }
                        j += 1;
                    } else if next.get("role").and_then(|r| r.as_str()) == Some("assistant")
                        && next.get("tool_calls").is_some()
                    {
                        // 下一个也是 assistant with tool_calls，停止
                        break;
                    } else {
                        break;
                    }
                }

                // 补缺失的 tool 消息
                let mut insertions: Vec<(usize, Value)> = Vec::new();
                for (idx, call_id) in required_ids.iter().enumerate() {
                    if !provided_ids.contains(call_id) {
                        log::debug!("[Transform] 补充缺失的 tool 消息: tool_call_id={}", call_id);
                        insertions.push((
                            i + 1 + idx,
                            json!({
                                "role": "tool",
                                "tool_call_id": call_id,
                                "content": "[tool call was made but no output was provided]"
                            }),
                        ));
                    }
                }

                // 逆序插入（避免索引偏移）
                for (pos, msg) in insertions.into_iter().rev() {
                    messages.insert(pos, msg);
                }
            }
        }
        i += 1;
    }
}

/// Responses tool_choice → Chat tool_choice
fn map_tool_choice_to_chat(tc: &Value) -> Value {
    match tc {
        Value::String(s) => match s.as_str() {
            "required" => json!("required"),
            "auto" => json!("auto"),
            "none" => json!("none"),
            other => json!(other),
        },
        Value::Object(obj) => {
            // Responses: {"type": "function", "name": "xxx"}
            // Chat: {"type": "function", "function": {"name": "xxx"}}
            if obj.get("type").and_then(|t| t.as_str()) == Some("function") {
                if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
                    return json!({
                        "type": "function",
                        "function": {"name": name}
                    });
                }
            }
            tc.clone()
        }
        _ => tc.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_responses_to_chat_simple() {
        let input = json!({
            "model": "gpt-4o",
            "instructions": "You are helpful.",
            "input": [{"type": "message", "role": "user", "content": "Hello"}],
            "max_output_tokens": 1024,
            "stream": false
        });
        let result = responses_to_chat_request(input).unwrap();
        assert_eq!(result["model"], "gpt-4o");
        assert_eq!(result["messages"][0]["role"], "system");
        assert_eq!(result["messages"][0]["content"], "You are helpful.");
        assert_eq!(result["messages"][1]["role"], "user");
        assert_eq!(result["messages"][1]["content"], "Hello");
        assert_eq!(result["max_tokens"], 1024);
    }

    #[test]
    fn test_responses_to_chat_with_function_call() {
        let input = json!({
            "model": "gpt-4o",
            "input": [
                {"type": "message", "role": "user", "content": "What's the weather?"},
                {"type": "function_call", "call_id": "call_1", "name": "get_weather", "arguments": "{\"loc\":\"Tokyo\"}"},
                {"type": "function_call_output", "call_id": "call_1", "output": "Sunny"}
            ]
        });
        let result = responses_to_chat_request(input).unwrap();
        let msgs = result["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[1]["role"], "assistant");
        assert_eq!(msgs[1]["tool_calls"][0]["id"], "call_1");
        assert_eq!(msgs[2]["role"], "tool");
        assert_eq!(msgs[2]["tool_call_id"], "call_1");
    }

    #[test]
    fn test_responses_to_chat_tools() {
        let input = json!({
            "model": "gpt-4o",
            "input": [],
            "tools": [{
                "type": "function",
                "name": "get_weather",
                "description": "Get weather",
                "parameters": {"type": "object", "properties": {"loc": {"type": "string"}}}
            }]
        });
        let result = responses_to_chat_request(input).unwrap();
        assert_eq!(result["tools"][0]["type"], "function");
        assert_eq!(result["tools"][0]["function"]["name"], "get_weather");
    }

    #[test]
    fn test_chat_to_responses_simple() {
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hello!"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        });
        let result = chat_to_responses_response(input).unwrap();
        assert_eq!(result["status"], "completed");
        assert_eq!(result["output"][0]["type"], "message");
        assert_eq!(result["output"][0]["content"][0]["text"], "Hello!");
        assert_eq!(result["usage"]["input_tokens"], 10);
    }

    #[test]
    fn test_chat_to_responses_with_tool_calls() {
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "get_weather", "arguments": "{\"loc\":\"Tokyo\"}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        });
        let result = chat_to_responses_response(input).unwrap();
        assert_eq!(result["status"], "completed");
        let fc = result["output"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["type"] == "function_call")
            .unwrap();
        assert_eq!(fc["call_id"], "call_1");
        assert_eq!(fc["name"], "get_weather");
    }

    #[test]
    fn test_chat_to_responses_with_reasoning() {
        let input = json!({
            "id": "chatcmpl-123",
            "model": "deepseek-v4",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "reasoning_content": "Let me think...",
                    "content": "The answer is 42."
                },
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 20}
        });
        let result = chat_to_responses_response(input).unwrap();
        assert_eq!(result["output"][0]["type"], "reasoning");
        assert_eq!(result["output"][1]["type"], "message");
        assert_eq!(
            result["output"][1]["content"][0]["text"],
            "The answer is 42."
        );
    }

    #[test]
    fn test_chat_to_responses_length_finish() {
        let input = json!({
            "id": "chatcmpl-123",
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Partial..."},
                "finish_reason": "length"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 4096}
        });
        let result = chat_to_responses_response(input).unwrap();
        assert_eq!(result["status"], "incomplete");
        assert_eq!(result["incomplete_details"]["reason"], "max_output_tokens");
    }

    #[test]
    fn test_tool_choice_function_to_chat() {
        let tc = json!({"type": "function", "name": "get_weather"});
        let result = map_tool_choice_to_chat(&tc);
        assert_eq!(result["type"], "function");
        assert_eq!(result["function"]["name"], "get_weather");
    }

    #[test]
    fn test_responses_to_chat_reasoning_effort() {
        let input = json!({
            "model": "gpt-5.4",
            "input": [],
            "reasoning": {"effort": "high"}
        });
        let result = responses_to_chat_request(input).unwrap();
        assert_eq!(result["reasoning_effort"], "high");
    }
}
