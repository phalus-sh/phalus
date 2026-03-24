use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::DocEntry;

#[derive(Debug, Error)]
pub enum DocSiteError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Response body too large (limit: {limit_kb} KB)")]
    TooLarge { limit_kb: u64 },
}

/// Remove HTML tags from `html`, stripping all content inside `<script>` and
/// `<style>` elements, converting block-level elements to newlines, and
/// collapsing runs of whitespace.
pub fn strip_html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut chars = html.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '<' {
            out.push(c);
            continue;
        }

        // We are inside a tag. Collect the tag name to decide how to handle it.
        let mut tag_buf = String::new();
        for ch in chars.by_ref() {
            if ch == '>' {
                break;
            }
            tag_buf.push(ch);
        }

        let tag_lower = tag_buf.trim_start_matches('/').to_lowercase();
        let tag_name: &str = tag_lower
            .split(|c: char| c.is_whitespace())
            .next()
            .unwrap_or("");

        // Skip the entire content of script/style elements.
        if tag_name == "script" || tag_name == "style" {
            // Consume until </script> or </style>
            let close = format!("</{}", tag_name);
            let mut buf = String::new();
            while let Some(ch) = chars.next() {
                buf.push(ch);
                if buf.to_lowercase().ends_with(&close) {
                    // consume the rest of the closing tag up to '>'
                    for ch2 in chars.by_ref() {
                        if ch2 == '>' {
                            break;
                        }
                    }
                    break;
                }
                // Keep buffer small
                if buf.len() > close.len() + 1 {
                    buf = buf[buf.len() - (close.len() + 1)..].to_string();
                }
            }
            continue;
        }

        // Block-level elements become newlines.
        let block_tags = [
            "p", "div", "br", "h1", "h2", "h3", "h4", "h5", "h6", "li", "tr", "td", "th",
            "blockquote", "pre", "article", "section", "header", "footer", "nav", "main",
        ];
        if block_tags.contains(&tag_name) {
            out.push('\n');
        }
        // All other tags are simply removed.
    }

    // Collapse runs of whitespace (preserve single newlines for readability)
    let mut result = String::with_capacity(out.len());
    let mut last_was_newline = false;
    let mut last_was_space = false;
    for ch in out.chars() {
        if ch == '\n' || ch == '\r' {
            if !last_was_newline {
                result.push('\n');
            }
            last_was_newline = true;
            last_was_space = false;
        } else if ch.is_whitespace() {
            if !last_was_space && !last_was_newline {
                result.push(' ');
            }
            last_was_space = true;
        } else {
            result.push(ch);
            last_was_newline = false;
            last_was_space = false;
        }
    }

    result.trim().to_string()
}

/// Strip fenced code blocks (``` … ```) that contain more than `max_lines`
/// lines of content.
pub fn strip_long_code_examples(text: &str, max_lines: usize) -> String {
    let mut out = String::new();
    let mut rest = text;

    while let Some(open_pos) = rest.find("```") {
        // Append everything before the fence.
        out.push_str(&rest[..open_pos]);

        // Move past the opening ```.
        let after_open = &rest[open_pos + 3..];

        // The opening fence line ends at the first newline.
        let block_start = match after_open.find('\n') {
            Some(nl) => nl + 1,
            None => {
                // Malformed – no newline after fence; keep as-is and stop.
                out.push_str("```");
                out.push_str(after_open);
                return out;
            }
        };
        let fence_info = &after_open[..block_start]; // includes the newline

        let block_body = &after_open[block_start..];

        // Find the closing ```.
        let Some(close_pos) = block_body.find("```") else {
            // No closing fence; keep everything remaining as-is.
            out.push_str("```");
            out.push_str(after_open);
            return out;
        };

        let content = &block_body[..close_pos];
        let line_count = content.lines().count();

        if line_count <= max_lines {
            // Keep the block.
            out.push_str("```");
            out.push_str(fence_info);
            out.push_str(content);
            out.push_str("```");
        }
        // else: strip the block entirely.

        // Advance past the closing ``` (and optional trailing characters on the same line).
        let after_close = &block_body[close_pos + 3..];
        // Consume the rest of the closing fence line.
        rest = match after_close.find('\n') {
            Some(nl) => &after_close[nl + 1..],
            None => "",
        };
    }

    out.push_str(rest);
    out
}

/// Fetch a documentation site URL, enforce a size limit, strip HTML, and
/// return a `DocEntry`.
pub async fn fetch_doc_site(url: &str, max_size_kb: u64) -> Result<DocEntry, DocSiteError> {
    let response = reqwest::get(url).await?.error_for_status()?;

    // Check Content-Length header first if available.
    if let Some(len) = response.content_length() {
        if len > max_size_kb * 1024 {
            return Err(DocSiteError::TooLarge {
                limit_kb: max_size_kb,
            });
        }
    }

    let bytes = response.bytes().await?;
    if bytes.len() as u64 > max_size_kb * 1024 {
        return Err(DocSiteError::TooLarge {
            limit_kb: max_size_kb,
        });
    }

    let raw = String::from_utf8_lossy(&bytes).into_owned();
    let content = strip_html_to_text(&raw);

    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let content_hash = format!("{:x}", hasher.finalize());

    // Derive a short name from the URL.
    let name = url
        .rsplit('/')
        .find(|s| !s.is_empty())
        .unwrap_or("doc")
        .to_string();

    Ok(DocEntry {
        name,
        content,
        source_url: Some(url.to_string()),
        content_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_tags() {
        let html = "<html><body><h1>Title</h1><p>Content here</p><script>evil()</script></body></html>";
        let text = strip_html_to_text(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Content here"));
        assert!(!text.contains("<h1>"));
        assert!(!text.contains("evil()"));
    }

    #[test]
    fn test_strip_code_examples() {
        let text = "Some text\n```\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\n```\nMore text";
        let stripped = strip_long_code_examples(text, 10);
        assert!(stripped.contains("Some text"));
        assert!(stripped.contains("More text"));
        assert!(!stripped.contains("line11"));
    }

    #[test]
    fn test_short_code_examples_kept() {
        let text = "Before\n```\nshort example\n```\nAfter";
        let stripped = strip_long_code_examples(text, 10);
        assert!(stripped.contains("short example"));
    }
}
