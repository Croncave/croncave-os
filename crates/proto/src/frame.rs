use bytes::{BufMut, Bytes, BytesMut};

/// What a frame means for its stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameKind {
    /// Opens a stream; payload is a JSON [`crate::StreamOpen`]. Only the relay opens streams.
    Open = 1,
    /// Bytes for an open stream.
    Data = 2,
    /// The sender has nothing more to send on this stream (half close).
    End = 3,
    /// The stream failed; payload is a UTF-8 reason. Closes both directions.
    Reset = 4,
    /// A JSON control message on stream 0.
    Control = 5,
}

impl TryFrom<u8> for FrameKind {
    type Error = FrameError;
    fn try_from(v: u8) -> Result<Self, FrameError> {
        Ok(match v {
            1 => Self::Open,
            2 => Self::Data,
            3 => Self::End,
            4 => Self::Reset,
            5 => Self::Control,
            other => return Err(FrameError::UnknownKind(other)),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: FrameKind,
    pub stream: u32,
    pub payload: Bytes,
}

#[derive(Debug)]
pub enum FrameError {
    TooShort,
    UnknownKind(u8),
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::TooShort => write!(f, "frame shorter than its 5-byte header"),
            FrameError::UnknownKind(k) => write!(f, "unknown frame kind {k}"),
        }
    }
}

impl std::error::Error for FrameError {}

pub const HEADER_LEN: usize = 5;

impl Frame {
    pub fn new(kind: FrameKind, stream: u32, payload: impl Into<Bytes>) -> Self {
        Self { kind, stream, payload: payload.into() }
    }

    pub fn control<T: serde::Serialize>(msg: &T) -> Self {
        let json = serde_json::to_vec(msg).expect("control messages always serialize");
        Self::new(FrameKind::Control, 0, json)
    }

    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(HEADER_LEN + self.payload.len());
        buf.put_u8(self.kind as u8);
        buf.put_u32(self.stream);
        buf.extend_from_slice(&self.payload);
        buf.freeze()
    }

    pub fn decode(raw: Bytes) -> Result<Self, FrameError> {
        if raw.len() < HEADER_LEN {
            return Err(FrameError::TooShort);
        }
        let kind = FrameKind::try_from(raw[0])?;
        let stream = u32::from_be_bytes([raw[1], raw[2], raw[3], raw[4]]);
        Ok(Self { kind, stream, payload: raw.slice(HEADER_LEN..) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_kind() {
        for kind in [FrameKind::Open, FrameKind::Data, FrameKind::End, FrameKind::Reset, FrameKind::Control] {
            let f = Frame::new(kind, 0xDEAD_BEEF, Bytes::from_static(b"hello"));
            let back = Frame::decode(f.encode()).unwrap();
            assert_eq!(back, f);
        }
    }

    #[test]
    fn rejects_short_and_unknown_frames() {
        assert!(matches!(Frame::decode(Bytes::from_static(b"\x02\x00")), Err(FrameError::TooShort)));
        assert!(matches!(Frame::decode(Bytes::from_static(b"\x09\x00\x00\x00\x01")), Err(FrameError::UnknownKind(9))));
    }

    #[test]
    fn empty_payload_is_allowed() {
        let f = Frame::new(FrameKind::End, 7, Bytes::new());
        assert_eq!(f.encode().len(), HEADER_LEN);
        assert_eq!(Frame::decode(f.encode()).unwrap().stream, 7);
    }
}
