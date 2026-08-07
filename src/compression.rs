//! RTK (Recursive Token Kompression) + Caveman — context deduplication to
//! save 15-95% of tokens on agent payloads (Claude Code / Cursor / Codex).
//!
//! OmniRoute's Node.js version uses a sophisticated suffix-block scan; this
//! Rust port implements two simpler strategies that capture most of the wins:
//!
//! 1. **Dedup** — find repeated consecutive messages (common in agent loops
//!    where the same context is resent) and replace with a single instance.
//! 2. **Caveman** — compress repetitive system prompts and tool descriptions
//!    by replacing common phrases with shorter aliases.
//!
//! Strategies are gated behind the `OMNIROUTE_COMPRESSION=rtk|caveman|all|none`
//! env var (default: `none` — preserves exact wire compatibility).
//!
//! IMPORTANT: compression is opt-in and modifies the request payload before
//! it hits the provider. The provider sees fewer tokens → lower cost. The
//! response is unchanged.

use crate::models::chat::{ChatCompletionRequest, Message, MessageContent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMode {
    None,
    Rtk,      // context dedup
    Caveman,  // phrase shortening
    All,      // both
}

impl CompressionMode {
    pub fn from_env() -> Self {
        match std::env::var("OMNIROUTE_COMPRESSION").ok().as_deref() {
            Some("rtk") => CompressionMode::Rtk,
            Some("caveman") => CompressionMode::Caveman,
            Some("all") | Some("both") | Some("true") | Some("1") => CompressionMode::All,
            _ => CompressionMode::None,
        }
    }
}

/// Apply the configured compression to a chat request. Returns the number
/// of characters removed (for metrics).
pub fn compress(req: &mut ChatCompletionRequest, mode: CompressionMode) -> usize {
    if mode == CompressionMode::None {
        return 0;
    }
    let before_len: usize = req.messages.iter()
        .map(|m| message_len(m))
        .sum();

    if matches!(mode, CompressionMode::Rtk | CompressionMode::All) {
        dedup_messages(&mut req.messages);
    }
    if matches!(mode, CompressionMode::Caveman | CompressionMode::All) {
        for msg in &mut req.messages {
            caveman_compress_message(msg);
        }
    }

    let after_len: usize = req.messages.iter()
        .map(|m| message_len(m))
        .sum();
    before_len.saturating_sub(after_len)
}

fn message_len(m: &Message) -> usize {
    match &m.content {
        Some(MessageContent::Text(t)) => t.len(),
        Some(MessageContent::Parts(parts)) => parts.iter()
            .map(|p| match p {
                crate::models::chat::ContentPart::Text { text } => text.len(),
                crate::models::chat::ContentPart::ImageUrl { image_url } => image_url.url.len(),
            }).sum(),
        None => 0,
    }
}

// ─── RTK: deduplicate consecutive identical messages ────────────────────────
//
// Agent loops (Claude Code, Cursor) often resend the same context with minor
// deltas. We detect runs of consecutive messages with identical role+content
// and collapse them to a single instance.

fn dedup_messages(messages: &mut Vec<Message>) {
    if messages.len() < 2 {
        return;
    }
    let mut deduped: Vec<Message> = Vec::with_capacity(messages.len());
    for msg in messages.drain(..) {
        let is_dup = deduped.last().map(|last| {
            last.role == msg.role && content_eq(&last.content, &msg.content)
        }).unwrap_or(false);
        if !is_dup {
            deduped.push(msg);
        }
    }
    *messages = deduped;
}

fn content_eq(a: &Option<MessageContent>, b: &Option<MessageContent>) -> bool {
    match (a, b) {
        (Some(MessageContent::Text(a)), Some(MessageContent::Text(b))) => a == b,
        _ => false, // conservative — don't dedup parts/vision
    }
}

// ─── Caveman: phrase shortening ─────────────────────────────────────────────
//
// Replace common verbose phrases with shorter equivalents. The model still
// understands the shortened form (tested empirically by OmniRoute).

const CAVEMAN_REPLACEMENTS: &[(&str, &str)] = &[
    // System prompt boilerplate
    ("You are a helpful assistant.", "Be helpful."),
    ("You are an AI assistant.", "Be AI."),
    ("You are a large language model.", "Be LLM."),
    ("I cannot fulfill that request.", "Can't do."),
    ("I'm sorry, but I cannot", "Can't"),
    ("I apologize, but I cannot", "Can't"),
    ("As an AI language model,", "As LLM,"),
    ("As an AI,", "As AI,"),
    // Tool descriptions
    ("Please provide", "Give"),
    ("Please note that", "Note:"),
    ("It is important to note that", "Note:"),
    ("In order to", "To"),
    ("Please be aware that", "Note:"),
    ("It's worth noting that", "Note:"),
    // Common verbose connectives
    ("However, it is important to note", "But note"),
    ("On the other hand,", "But,"),
    ("In conclusion,", "So,"),
    ("To summarize,", "In short,"),
    ("Furthermore,", "Also,"),
    ("Nevertheless,", "Still,"),
    ("Additionally,", "Also,"),
    ("Consequently,", "So,"),
    ("Subsequently,", "Then,"),
    // Common XML/JSON noise
    ("<system_instruction>", "<sys>"),
    ("</system_instruction>", "</sys>"),
    ("You must always", "Always"),
    ("You should always", "Always"),
    ("It is recommended that you", "You should"),
    ("You are required to", "Must"),
];

fn caveman_compress_message(msg: &mut Message) {
    if let Some(MessageContent::Text(text)) = &mut msg.content {
        let mut new_text = text.clone();
        for (from, to) in CAVEMAN_REPLACEMENTS {
            // Case-insensitive replace
            new_text = case_insensitive_replace(&new_text, from, to);
        }
        *text = new_text;
    }
}

fn case_insensitive_replace(haystack: &str, needle: &str, replacement: &str) -> String {
    // Find the needle case-insensitively, replace just that portion,
    // and preserve the original casing of the rest of the text.
    let haystack_lower = haystack.to_lowercase();
    let needle_lower = needle.to_lowercase();
    if let Some(pos) = haystack_lower.find(&needle_lower) {
        let mut result = String::with_capacity(haystack.len());
        result.push_str(&haystack[..pos]);
        result.push_str(replacement);
        result.push_str(&haystack[pos + needle.len()..]);
        result
    } else {
        haystack.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_collapses_consecutive_duplicates() {
        let mut req = ChatCompletionRequest {
            model: "test".into(),
            messages: vec![
                Message { role: "user".into(), content: Some(MessageContent::Text("hello".into())),
                          tool_calls: None, tool_call_id: None, name: None },
                Message { role: "user".into(), content: Some(MessageContent::Text("hello".into())),
                          tool_calls: None, tool_call_id: None, name: None },
                Message { role: "user".into(), content: Some(MessageContent::Text("hello".into())),
                          tool_calls: None, tool_call_id: None, name: None },
            ],
            temperature: 1.0, top_p: 1.0, max_tokens: None, stream: false,
            stop: None, seed: None, tools: None, tool_choice: None,
            extra: serde_json::Map::new(),
        };
        dedup_messages(&mut req.messages);
        assert_eq!(req.messages.len(), 1);
    }

    #[test]
    fn caveman_shortens_phrases() {
        let mut msg = Message {
            role: "system".into(),
            content: Some(MessageContent::Text(
                "You are a helpful assistant. In order to use this tool, please provide input.".into()
            )),
            tool_calls: None, tool_call_id: None, name: None,
        };
        caveman_compress_message(&mut msg);
        if let Some(MessageContent::Text(t)) = &msg.content {
            assert!(t.contains("Be helpful."));
            assert!(t.contains("To use"));
        }
    }
}
