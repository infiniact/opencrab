use anyhow::Result;

use super::client::FeishuClient;
use crate::types::OutboundMessage;

/// Maximum text length per Feishu message (4000 chars).
const MAX_TEXT_LEN: usize = 4000;

/// Send a message through Feishu, splitting long text into chunks.
pub async fn send_message(client: &FeishuClient, msg: &OutboundMessage) -> Result<()> {
    let chunks = split_text(&msg.text, MAX_TEXT_LEN);

    for (i, chunk) in chunks.iter().enumerate() {
        // Reply to original message for the first chunk, send normally for subsequent.
        if i == 0 {
            if let Some(ref reply_to) = msg.reply_to {
                client.reply_text(reply_to, chunk).await?;
            } else {
                client.send_text(&msg.chat_id, chunk).await?;
            }
        } else {
            client.send_text(&msg.chat_id, chunk).await?;
        }
    }

    Ok(())
}

/// Split text into chunks, preferring to break at newlines.
fn split_text(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_len {
            chunks.push(remaining.to_string());
            break;
        }

        // Try to find a newline to break at.
        let slice = &remaining[..max_len];
        let break_at = slice.rfind('\n').unwrap_or(max_len);
        let break_at = if break_at == 0 { max_len } else { break_at };

        chunks.push(remaining[..break_at].to_string());
        remaining = remaining[break_at..].trim_start_matches('\n');
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_short_text() {
        let chunks = split_text("hello", 4000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello");
    }

    #[test]
    fn split_long_text() {
        let text = "a".repeat(5000);
        let chunks = split_text(&text, 4000);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), 4000);
        assert_eq!(chunks[1].len(), 1000);
    }

    #[test]
    fn split_at_newline() {
        let text = format!("{}\n{}", "a".repeat(3000), "b".repeat(2000));
        let chunks = split_text(&text, 4000);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "a".repeat(3000));
        assert_eq!(chunks[1], "b".repeat(2000));
    }
}
