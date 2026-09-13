use std::io::{BufRead, BufReader, Read, Write};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

const MAX_MCP_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageFraming {
    Newline,
    ContentLength,
}

#[derive(Debug)]
pub struct IncomingMessage {
    pub payload: Value,
    pub framing: MessageFraming,
}

pub fn read_message<R>(reader: &mut BufReader<R>) -> Result<Option<IncomingMessage>>
where
    R: Read,
{
    let mut content_length = None;

    loop {
        let Some(line) = read_line_limited(reader, "MCP message")? else {
            return Ok(None);
        };

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            if trimmed.len() > MAX_MCP_MESSAGE_BYTES {
                bail!("MCP message exceeds the 16 MiB limit");
            }
            let value = serde_json::from_str(trimmed)
                .context("failed to parse newline-delimited MCP JSON-RPC message")?;
            return Ok(Some(IncomingMessage {
                payload: value,
                framing: MessageFraming::Newline,
            }));
        }

        let mut is_content_length = false;
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                let parsed = value
                    .trim()
                    .parse::<usize>()
                    .context("invalid Content-Length header")?;
                if parsed > MAX_MCP_MESSAGE_BYTES {
                    bail!("MCP message exceeds the 16 MiB limit");
                }
                content_length = Some(parsed);
                is_content_length = true;
            }
        }
        if is_content_length {
            break;
        }

        if trimmed.split_once(':').is_some() {
            break;
        }

        bail!("invalid MCP message start");
    }

    loop {
        let Some(line) = read_line_limited(reader, "MCP header")? else {
            bail!("unexpected EOF while reading MCP headers");
        };

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }

        if let Some((name, value)) = trimmed.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                let parsed = value
                    .trim()
                    .parse::<usize>()
                    .context("invalid Content-Length header")?;
                if parsed > MAX_MCP_MESSAGE_BYTES {
                    bail!("MCP message exceeds the 16 MiB limit");
                }
                content_length = Some(parsed);
            }
        }
    }

    let content_length = content_length.ok_or_else(|| anyhow!("missing Content-Length header"))?;
    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .context("failed to read MCP message body")?;

    let value =
        serde_json::from_slice(&body).context("failed to parse MCP JSON-RPC message body")?;
    Ok(Some(IncomingMessage {
        payload: value,
        framing: MessageFraming::ContentLength,
    }))
}

fn read_line_limited<R>(reader: &mut BufReader<R>, label: &str) -> Result<Option<String>>
where
    R: Read,
{
    let mut bytes = Vec::new();
    loop {
        let buffer = reader
            .fill_buf()
            .with_context(|| format!("failed to read {label}"))?;
        if buffer.is_empty() {
            if bytes.is_empty() {
                return Ok(None);
            }
            break;
        }

        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(buffer.len(), |index| index + 1);
        if bytes.len() + consumed > MAX_MCP_MESSAGE_BYTES + 2 {
            bail!("MCP message exceeds the 16 MiB limit");
        }
        bytes.extend_from_slice(&buffer[..consumed]);
        reader.consume(consumed);

        if newline.is_some() {
            break;
        }
    }

    String::from_utf8(bytes)
        .map(Some)
        .context("MCP message is not valid UTF-8")
}

pub fn write_message<W>(writer: &mut W, payload: &Value, framing: MessageFraming) -> Result<()>
where
    W: Write,
{
    let body = serde_json::to_vec(payload).context("failed to serialize MCP response")?;
    match framing {
        MessageFraming::Newline => {
            writer
                .write_all(&body)
                .context("failed to write MCP response body")?;
            writer
                .write_all(b"\n")
                .context("failed to write MCP response newline")?;
        }
        MessageFraming::ContentLength => {
            write!(writer, "Content-Length: {}\r\n\r\n", body.len())
                .context("failed to write MCP response header")?;
            writer
                .write_all(&body)
                .context("failed to write MCP response body")?;
        }
    }
    writer.flush().context("failed to flush MCP response")?;
    Ok(())
}
