use std::io::{self, Read, Write};

use serde::Deserialize;
use serde_json::{json, Value};

pub const PROTOCOL_MAJOR: u16 = 0;
pub const PROTOCOL_MINOR: u16 = 0;
pub const DELTA_LIMIT: usize = 8;
pub const MAX_FRAME_BYTES: u32 = 1024 * 1024;
pub const SOCKET_FILE_NAME: &str = "keeplined.sock";
pub const INTENT_DIR_NAME: &str = "intents";

pub(crate) const MIN_DIMENSION: u16 = 2;
pub(crate) const MAX_COLUMNS: u16 = 400;
pub(crate) const MAX_ROWS: u16 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PullClass {
    Ahead,
    Current,
    Deltas,
    Resync,
}

pub(crate) fn classify_pull(oldest: Option<u64>, current: u64, after: u64) -> PullClass {
    if after > current {
        PullClass::Ahead
    } else if after == current {
        PullClass::Current
    } else {
        match oldest {
            Some(oldest) if after + 1 >= oldest => PullClass::Deltas,
            _ => PullClass::Resync,
        }
    }
}

pub(crate) fn read_frame(stream: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf);
    if len == 0 || len > MAX_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame length {len} is outside 1..={MAX_FRAME_BYTES}"),
        ));
    }
    let mut buf = vec![0u8; len as usize];
    stream.read_exact(&mut buf)?;
    Ok(buf)
}

pub(crate) fn write_frame(stream: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    if payload.len() > MAX_FRAME_BYTES as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "response exceeds the frame limit",
        ));
    }
    let len = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "response length overflow"))?;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(payload)?;
    stream.flush()?;
    Ok(())
}

pub(crate) fn ok_response(id: &str, result: Value) -> Value {
    json!({
        "id": id,
        "major": PROTOCOL_MAJOR,
        "minor": PROTOCOL_MINOR,
        "experimental": true,
        "ok": true,
        "result": result,
    })
}

pub(crate) fn error_response(id: &str, code: &str, message: impl Into<String>) -> Value {
    json!({
        "id": id,
        "major": PROTOCOL_MAJOR,
        "minor": PROTOCOL_MINOR,
        "experimental": true,
        "ok": false,
        "error": {
            "code": code,
            "message": message.into(),
        },
    })
}

#[derive(Debug, Deserialize)]
pub(crate) struct Request {
    pub id: String,
    pub op: String,
    pub major: Option<u16>,
    pub operation_id: Option<String>,
    pub argv: Option<Vec<String>>,
    pub cwd: Option<String>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
    pub terminal_id: Option<String>,
    pub after_revision: Option<u64>,
    pub generation: Option<u64>,
    pub token: Option<String>,
    pub data: Option<String>,
}

pub(crate) fn decode_request(frame: &[u8]) -> Result<Request, Value> {
    let value: Value = serde_json::from_slice(frame)
        .map_err(|err| error_response("", "bad_frame", format!("request is not JSON: {err}")))?;
    let id = match value.get("id").and_then(Value::as_str) {
        Some(id) if !id.is_empty() && id.len() <= 128 => id.to_owned(),
        Some(_) => {
            return Err(error_response(
                "",
                "bad_frame",
                "request id must be 1 to 128 characters",
            ));
        }
        None => {
            return Err(error_response("", "bad_frame", "request id is required"));
        }
    };
    serde_json::from_value(value).map_err(|err| {
        error_response(
            &id,
            "bad_frame",
            format!("request fields are invalid: {err}"),
        )
    })
}

pub(crate) fn is_disconnect(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::BrokenPipe
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::UnexpectedEof
            | io::ErrorKind::ConnectionAborted
    )
}

#[cfg(test)]
mod tests {
    use super::{classify_pull, PullClass};

    #[test]
    fn revision_gap_is_explicit() {
        assert_eq!(classify_pull(Some(7), 14, 0), PullClass::Resync);
        assert_eq!(classify_pull(Some(7), 14, 6), PullClass::Deltas);
        assert_eq!(classify_pull(Some(7), 14, 14), PullClass::Current);
        assert_eq!(classify_pull(Some(7), 14, 15), PullClass::Ahead);
        assert_eq!(classify_pull(None, 4, 0), PullClass::Resync);
        assert_eq!(classify_pull(None, 0, 0), PullClass::Current);
    }
}
