use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::constants::{VIDEO_INIT_FRAME, VIDEO_JPEG_FRAME, VIDEO_MEDIA_FRAME};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VideoFrameType {
    Init,
    Media,
    Jpeg,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    pub frame_type: VideoFrameType,
    pub stream_id: u32,
    pub sequence: u64,
    pub timestamp_ms: u64,
    pub payload: Bytes,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("video frame too short")]
    TooShort,
    #[error("unknown video frame type: {0}")]
    UnknownType(u8),
}

pub fn encode_video_frame(frame: &VideoFrame) -> Bytes {
    let mut out = BytesMut::with_capacity(21 + frame.payload.len());
    out.put_u8(match frame.frame_type {
        VideoFrameType::Init => VIDEO_INIT_FRAME,
        VideoFrameType::Media => VIDEO_MEDIA_FRAME,
        VideoFrameType::Jpeg => VIDEO_JPEG_FRAME,
    });
    out.put_u32(frame.stream_id);
    out.put_u64(frame.sequence);
    out.put_u64(frame.timestamp_ms);
    out.extend_from_slice(&frame.payload);
    out.freeze()
}

pub fn decode_video_frame(mut bytes: Bytes) -> Result<VideoFrame, FrameError> {
    if bytes.len() < 21 {
        return Err(FrameError::TooShort);
    }

    let raw_type = bytes.get_u8();
    let frame_type = match raw_type {
        VIDEO_INIT_FRAME => VideoFrameType::Init,
        VIDEO_MEDIA_FRAME => VideoFrameType::Media,
        VIDEO_JPEG_FRAME => VideoFrameType::Jpeg,
        other => return Err(FrameError::UnknownType(other)),
    };

    Ok(VideoFrame {
        frame_type,
        stream_id: bytes.get_u32(),
        sequence: bytes.get_u64(),
        timestamp_ms: bytes.get_u64(),
        payload: bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_video_frame() {
        let frame = VideoFrame {
            frame_type: VideoFrameType::Media,
            stream_id: 7,
            sequence: 42,
            timestamp_ms: 1234,
            payload: Bytes::from_static(b"segment"),
        };

        let decoded = decode_video_frame(encode_video_frame(&frame)).unwrap();

        assert_eq!(decoded, frame);
    }

    #[test]
    fn round_trips_jpeg_frame() {
        let frame = VideoFrame {
            frame_type: VideoFrameType::Jpeg,
            stream_id: 0,
            sequence: 1,
            timestamp_ms: 5678,
            payload: Bytes::from_static(b"\xff\xd8\xff"),
        };

        let decoded = decode_video_frame(encode_video_frame(&frame)).unwrap();

        assert_eq!(decoded, frame);
    }
}
