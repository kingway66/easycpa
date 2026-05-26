//! OpenAI Chat Completions SSE → OpenAI Responses SSE 流式转换
//!
//! 当 Codex 客户端期望 Responses SSE 事件格式，但上游返回 Chat SSE 时使用。
//!
//! Chat SSE 格式:
//!   data: {"choices":[{"delta":{"content":"Hello"}}]}
//!
//! Responses SSE 格式:
//!   event: response.output_text.delta
//!   data: {"type":"response.output_text.delta","output_index":0,"content_index":0,"delta":"Hello"}

use crate::proxy::sse::{strip_sse_field, take_sse_block};
use bytes::Bytes;
use futures::stream::{Stream, StreamExt};
use serde_json::{json, Value};

/// 状态：跟踪当前活跃的输出项
struct StreamState {
    /// 当前文本内容是否已经开始（需要发送 done 事件）
    text_started: bool,
    /// 当前文本 output_index
    text_output_index: u64,
    /// 当前文本 content_index
    text_content_index: u64,
    /// 活跃的 tool call 索引 → call_id
    active_tool_calls: std::collections::HashMap<usize, ToolCallState>,
    /// 下一个 output_index
    next_output_index: u64,
    /// 已发送 response.created
    sent_created: bool,
    /// 已发送 response.completed
    sent_completed: bool,
    /// 收集的 usage
    usage: Option<Value>,
    /// 收集的完整文本（用于非流式回退）
    collected_text: String,
    /// 收集的 reasoning
    collected_reasoning: String,
    /// 已完成的 output items（用于填充 response.completed 的 output）
    completed_output_items: Vec<Value>,
    /// 最终 finish_reason
    finish_reason: Option<String>,
    /// 模型名
    model: String,
    /// 响应 ID
    response_id: String,
}

struct ToolCallState {
    call_id: String,
    name: String,
    arguments: String,
    output_index: u64,
}

impl StreamState {
    fn new() -> Self {
        Self {
            text_started: false,
            text_output_index: 0,
            text_content_index: 0,
            active_tool_calls: std::collections::HashMap::new(),
            next_output_index: 0,
            sent_created: false,
            sent_completed: false,
            usage: None,
            collected_text: String::new(),
            collected_reasoning: String::new(),
            completed_output_items: Vec::new(),
            finish_reason: None,
            model: String::new(),
            response_id: String::new(),
        }
    }
}

/// 创建 Chat SSE → Responses SSE 流式转换器
pub fn create_responses_sse_stream_from_chat(
    stream: impl Stream<Item = Result<Bytes, std::io::Error>> + Send + Unpin + 'static,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send + Unpin {
    let inner = stream.boxed();
    ChatToResponsesStream {
        inner,
        buffer: String::new(),
        remainder: Vec::new(),
        state: StreamState::new(),
    }
}

struct ChatToResponsesStream {
    inner: futures::stream::BoxStream<'static, Result<Bytes, std::io::Error>>,
    buffer: String,
    remainder: Vec<u8>,
    state: StreamState,
}

impl Stream for ChatToResponsesStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            // Try to extract a complete SSE block from the buffer
            if let Some(block) = take_sse_block(&mut self.buffer) {
                if let Some(bytes) = self.process_block(&block) {
                    return std::task::Poll::Ready(Some(Ok(bytes)));
                }
                continue; // block processed, try next
            }

            // Need more data from upstream
            match self.inner.poll_next_unpin(cx) {
                std::task::Poll::Ready(Some(Ok(chunk))) => {
                    // Prepend any incomplete UTF-8 bytes from previous chunk
                    if self.remainder.is_empty() {
                        self.buffer.push_str(&String::from_utf8_lossy(&chunk));
                    } else {
                        let mut combined = std::mem::take(&mut self.remainder);
                        combined.extend_from_slice(&chunk);
                        match std::str::from_utf8(&combined) {
                            Ok(s) => self.buffer.push_str(s),
                            Err(e) => {
                                self.buffer.push_str(&String::from_utf8_lossy(
                                    &combined[..e.valid_up_to()],
                                ));
                                self.remainder = combined[e.valid_up_to()..].to_vec();
                            }
                        }
                    }
                    continue; // try to extract blocks again
                }
                std::task::Poll::Ready(Some(Err(e))) => {
                    return std::task::Poll::Ready(Some(Err(e)));
                }
                std::task::Poll::Ready(None) => {
                    // Stream ended — flush remaining buffer and emit final events
                    if !self.buffer.trim().is_empty() {
                        let remaining = std::mem::take(&mut self.buffer);
                        for block in remaining.split("\n\n").filter(|b| !b.trim().is_empty()) {
                            let _ = self.process_block(block);
                        }
                    }
                    return self.emit_final_events();
                }
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    }
}

impl ChatToResponsesStream {
    fn process_block(&mut self, block: &str) -> Option<Bytes> {
        let mut event_name = "";
        let mut data_str = "";

        for line in block.lines() {
            if let Some(d) = strip_sse_field(line, "data") {
                data_str = d;
            }
            if let Some(e) = strip_sse_field(line, "event") {
                event_name = e;
            }
        }

        if data_str.is_empty() {
            return None;
        }

        // Skip [DONE]
        if data_str.trim() == "[DONE]" {
            return None;
        }

        let data: Value = match serde_json::from_str(data_str) {
            Ok(v) => v,
            Err(_) => return None,
        };

        // Skip non-Chat events
        if event_name.starts_with("response.") {
            // Already a Responses event — passthrough
            return Some(Bytes::from(format!(
                "event: {event_name}\ndata: {data_str}\n\n"
            )));
        }

        self.process_chat_chunk(&data)
    }

    fn process_chat_chunk(&mut self, data: &Value) -> Option<Bytes> {
        let mut events = Vec::new();

        // Extract model and id
        if self.state.response_id.is_empty() {
            self.state.response_id = data
                .get("id")
                .and_then(|i| i.as_str())
                .unwrap_or("chatcmpl-unknown")
                .to_string();
        }
        if self.state.model.is_empty() {
            self.state.model = data
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
        }

        // Emit response.created on first chunk
        if !self.state.sent_created {
            self.state.sent_created = true;
            events.push(format_sse_event(
                "response.created",
                &json!({
                    "type": "response.created",
                    "response": {
                        "id": self.state.response_id,
                        "object": "response",
                        "status": "in_progress",
                        "model": self.state.model,
                        "output": []
                    }
                }),
            ));
            events.push(format_sse_event(
                "response.in_progress",
                &json!({
                    "type": "response.in_progress",
                    "response": {
                        "id": self.state.response_id,
                        "object": "response",
                        "status": "in_progress",
                        "model": self.state.model,
                        "output": []
                    }
                }),
            ));
        }

        if let Some(choices) = data.get("choices").and_then(|c| c.as_array()) {
            for choice in choices {
                let delta = choice.get("delta")?;
                let finish_reason = choice.get("finish_reason").and_then(|r| r.as_str());

                // Text content
                if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                    if !content.is_empty() {
                        if !self.state.text_started {
                            self.state.text_started = true;
                            self.state.text_output_index = self.state.next_output_index;
                            self.state.next_output_index += 1;
                            events.push(format_sse_event(
                                "response.output_item.added",
                                &json!({
                                    "type": "response.output_item.added",
                                    "output_index": self.state.text_output_index,
                                    "item": {
                                        "type": "message",
                                        "id": format!("msg_{}", self.state.text_output_index),
                                        "role": "assistant",
                                        "status": "in_progress",
                                        "content": []
                                    }
                                }),
                            ));
                            events.push(format_sse_event(
                                "response.content_part.added",
                                &json!({
                                    "type": "response.content_part.added",
                                    "output_index": self.state.text_output_index,
                                    "content_index": self.state.text_content_index,
                                    "part": {"type": "output_text", "text": ""}
                                }),
                            ));
                        }
                        self.state.collected_text.push_str(content);
                        events.push(format_sse_event(
                            "response.output_text.delta",
                            &json!({
                                "type": "response.output_text.delta",
                                "output_index": self.state.text_output_index,
                                "content_index": self.state.text_content_index,
                                "delta": content
                            }),
                        ));
                    }
                }

                // Reasoning content (DeepSeek etc.)
                if let Some(reasoning) = delta
                    .get("reasoning_content")
                    .or_else(|| delta.get("reasoning"))
                    .and_then(|r| r.as_str())
                {
                    if !reasoning.is_empty() {
                        self.state.collected_reasoning.push_str(reasoning);
                    }
                }

                // Tool calls
                if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                    for tc in tool_calls {
                        let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                        let call_id = tc.get("id").and_then(|i| i.as_str());
                        let name = tc
                            .get("function")
                            .and_then(|f| f.get("name"))
                            .and_then(|n| n.as_str());
                        let arguments = tc
                            .get("function")
                            .and_then(|f| f.get("arguments"))
                            .and_then(|a| a.as_str());

                        if let Some(id) = call_id {
                            // New tool call — emit added event
                            let output_index = self.state.next_output_index;
                            self.state.next_output_index += 1;
                            self.state.active_tool_calls.insert(
                                index,
                                ToolCallState {
                                    call_id: id.to_string(),
                                    name: name.unwrap_or("").to_string(),
                                    arguments: String::new(),
                                    output_index,
                                },
                            );
                            events.push(format_sse_event(
                                "response.output_item.added",
                                &json!({
                                    "type": "response.output_item.added",
                                    "output_index": output_index,
                                    "item": {
                                        "type": "function_call",
                                        "id": id,
                                        "call_id": id,
                                        "name": name.unwrap_or(""),
                                        "arguments": "",
                                        "status": "in_progress"
                                    }
                                }),
                            ));
                        }

                        if let Some(args) = arguments {
                            if let Some(tc_state) = self.state.active_tool_calls.get_mut(&index) {
                                tc_state.arguments.push_str(args);
                                events.push(format_sse_event(
                                    "response.function_call_arguments.delta",
                                    &json!({
                                        "type": "response.function_call_arguments.delta",
                                        "output_index": tc_state.output_index,
                                        "delta": args
                                    }),
                                ));
                            }
                        }
                    }
                }

                // Finish reason
                if let Some(reason) = finish_reason {
                    self.state.finish_reason = Some(reason.to_string());

                    // Emit reasoning as output item (DeepSeek reasoning_content / OpenRouter reasoning)
                    // Always emit reasoning item BEFORE the message item, matching OpenAI format
                    if !self.state.collected_reasoning.is_empty() {
                        let reasoning_output_index = self.state.next_output_index;
                        self.state.next_output_index += 1;
                        let reasoning_item = json!({
                            "type": "reasoning",
                            "id": format!("rs_{}", reasoning_output_index),
                            "summary": [{"type": "summary_text", "text": self.state.collected_reasoning}]
                        });
                        events.push(format_sse_event(
                            "response.output_item.added",
                            &json!({
                                "type": "response.output_item.added",
                                "output_index": reasoning_output_index,
                                "item": {
                                    "type": "reasoning",
                                    "id": format!("rs_{}", reasoning_output_index),
                                    "summary": [{"type": "summary_text", "text": ""}]
                                }
                            }),
                        ));
                        events.push(format_sse_event(
                            "response.output_item.done",
                            &json!({
                                "type": "response.output_item.done",
                                "output_index": reasoning_output_index,
                                "item": reasoning_item
                            }),
                        ));
                        self.state.completed_output_items.push(reasoning_item);
                    }

                    // Close text item if started
                    if self.state.text_started {
                        let text_item = json!({
                            "type": "message",
                            "id": format!("msg_{}", self.state.text_output_index),
                            "role": "assistant",
                            "status": "completed",
                            "content": [{"type": "output_text", "text": self.state.collected_text}]
                        });
                        events.push(format_sse_event(
                            "response.content_part.done",
                            &json!({
                                "type": "response.content_part.done",
                                "output_index": self.state.text_output_index,
                                "content_index": self.state.text_content_index,
                                "part": {"type": "output_text", "text": self.state.collected_text}
                            }),
                        ));
                        events.push(format_sse_event(
                            "response.output_item.done",
                            &json!({
                                "type": "response.output_item.done",
                                "output_index": self.state.text_output_index,
                                "item": text_item
                            }),
                        ));
                        self.state.completed_output_items.push(text_item);
                    }

                    // Close tool call items (check before deciding on message item)
                    let tool_indices: Vec<usize> =
                        self.state.active_tool_calls.keys().copied().collect();
                    for idx in tool_indices {
                        if let Some(tc_state) = self.state.active_tool_calls.remove(&idx) {
                            let tc_item = json!({
                                "type": "function_call",
                                "id": tc_state.call_id,
                                "call_id": tc_state.call_id,
                                "name": tc_state.name,
                                "arguments": tc_state.arguments,
                                "status": "completed"
                            });
                            events.push(format_sse_event(
                                "response.output_item.done",
                                &json!({
                                    "type": "response.output_item.done",
                                    "output_index": tc_state.output_index,
                                    "item": tc_item
                                }),
                            ));
                            self.state.completed_output_items.push(tc_item);
                        }
                    }

                    // Only emit a fallback message item when there's NO text AND NO tool calls.
                    // If tool calls exist, Codex handles the turn via function_call items —
                    // injecting a reasoning-as-message item would corrupt the next request's
                    // messages sequence (DeepSeek expects tool_result after tool_calls).
                    if !self.state.text_started
                        && self.state.active_tool_calls.is_empty()
                        && !self.state.completed_output_items.iter().any(|item| {
                            item.get("type").and_then(|t| t.as_str()) == Some("function_call")
                        })
                        && !self.state.collected_reasoning.is_empty()
                    {
                        let msg_output_index = self.state.next_output_index;
                        self.state.next_output_index += 1;
                        let msg_item = json!({
                            "type": "message",
                            "id": format!("msg_{}", msg_output_index),
                            "role": "assistant",
                            "status": "completed",
                            "content": [{"type": "output_text", "text": self.state.collected_reasoning}]
                        });
                        events.push(format_sse_event(
                            "response.output_item.added",
                            &json!({
                                "type": "response.output_item.added",
                                "output_index": msg_output_index,
                                "item": {
                                    "type": "message",
                                    "id": format!("msg_{}", msg_output_index),
                                    "role": "assistant",
                                    "status": "in_progress",
                                    "content": []
                                }
                            }),
                        ));
                        events.push(format_sse_event(
                            "response.output_item.done",
                            &json!({
                                "type": "response.output_item.done",
                                "output_index": msg_output_index,
                                "item": msg_item
                            }),
                        ));
                        self.state.completed_output_items.push(msg_item);
                    }
                }
            }
        }

        // Usage
        if let Some(usage) = data.get("usage") {
            self.state.usage = Some(usage.clone());
        }

        if events.is_empty() {
            return None;
        }

        Some(Bytes::from(events.join("")))
    }

    fn emit_final_events(&mut self) -> std::task::Poll<Option<Result<Bytes, std::io::Error>>> {
        // Already emitted final events — signal stream end
        if self.state.sent_completed {
            return std::task::Poll::Ready(None);
        }

        let mut events = Vec::new();

        // If we never sent created, stream was empty — send minimal response
        if !self.state.sent_created {
            return std::task::Poll::Ready(None);
        }

        // Build usage
        let usage = self.state.usage.clone().unwrap_or(json!({
            "input_tokens": 0,
            "output_tokens": 0
        }));
        let input_tokens = usage
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output_tokens = usage
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let mut resp_usage = json!({
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
            "total_tokens": input_tokens + output_tokens
        });
        if let Some(cached) = usage
            .pointer("/prompt_tokens_details/cached_tokens")
            .and_then(|v| v.as_u64())
        {
            resp_usage["input_tokens_details"] = json!({"cached_tokens": cached});
        }

        let status = match self.state.finish_reason.as_deref() {
            Some("length") => "incomplete",
            _ => "completed",
        };
        let incomplete_details = if self.state.finish_reason.as_deref() == Some("length") {
            Some(json!({"reason": "max_output_tokens"}))
        } else {
            None
        };

        let mut response_obj = json!({
            "id": self.state.response_id,
            "object": "response",
            "status": status,
            "model": self.state.model,
            "output": self.state.completed_output_items,
            "usage": resp_usage
        });
        if let Some(details) = incomplete_details {
            response_obj["incomplete_details"] = details;
        }

        events.push(format_sse_event(
            "response.completed",
            &json!({
                "type": "response.completed",
                "response": response_obj
            }),
        ));

        self.state.sent_completed = true;
        std::task::Poll::Ready(Some(Ok(Bytes::from(events.join("")))))
    }
}

fn format_sse_event(event_name: &str, data: &Value) -> String {
    let data_str = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_string());
    format!("event: {event_name}\ndata: {data_str}\n\n")
}
