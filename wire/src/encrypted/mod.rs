use bytes::Bytes;
use ql_codec::Reader;
use ql_common::StreamId;

use crate::{
    encrypted_message::EncryptedMessage, Error, Nonce, QlCrypto, SessionHeader, SessionKey,
};

mod ack;
mod builder;
mod close;
mod stream_data;
mod stream_reset;
mod stream_window;

pub use ack::*;
pub use builder::*;
pub use close::*;
pub use stream_data::*;
pub use stream_reset::*;
pub use stream_window::*;

#[derive(Debug, Clone, PartialEq, Eq, ql_codec::Codec)]
#[codec(frozen, discriminants = SessionFrameKind)]
#[repr(u8)]
pub enum SessionFrame<B> {
    // todo: do we need ping as explicit frame?
    Ping = 1,
    Ack(RecordAck) = 2,
    StreamData(StreamData<B>) = 3,
    StreamWindow(StreamWindow) = 4,
    StreamReset(StreamReset) = 5,
    Close(SessionClose) = 6,
    Unpair = 7,
}

impl SessionFrameRef<Bytes, Vec<u8>> {
    fn into_frame(self) -> SessionFrame<Bytes> {
        match self {
            Self::Ping => SessionFrame::Ping,
            Self::Unpair => SessionFrame::Unpair,
            Self::Ack(frame) => SessionFrame::Ack(frame),
            Self::StreamData(frame) => SessionFrame::StreamData(frame),
            Self::StreamWindow(frame) => SessionFrame::StreamWindow(frame.into_owned()),
            Self::StreamReset(frame) => SessionFrame::StreamReset(frame.into_owned()),
            Self::Close(frame) => SessionFrame::Close(frame.into_owned()),
        }
    }
}

/// Parses the frames in `bytes`, slicing their stream data out of it without copying
pub fn parse_session_frames(bytes: &Bytes) -> SessionFrameIter {
    SessionFrameIter {
        reader: Reader::new(bytes.clone()),
    }
}

pub fn decode_session_frames(bytes: &[u8]) -> Result<Vec<SessionFrame<Vec<u8>>>, Error> {
    let mut reader = Reader::new(bytes);
    let mut frames = Vec::new();
    while !reader.is_empty() {
        frames.push(reader.decode()?);
    }
    Ok(frames)
}

#[derive(Clone)]
pub struct SessionFrameIter {
    reader: Reader<Bytes>,
}

impl Iterator for SessionFrameIter {
    type Item = Result<SessionFrame<Bytes>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.reader.is_empty() {
            return None;
        }
        let frame = self.reader.decode_ref::<SessionFrame<Vec<u8>>>();
        Some(frame.map(SessionFrameRef::into_frame).map_err(Into::into))
    }
}

pub fn decrypt_record<B: AsMut<[u8]>>(
    crypto: &impl QlCrypto,
    record_header: &crate::RecordHeader,
    header: &SessionHeader,
    encrypted: EncryptedMessage<B>,
    session_key: &SessionKey,
) -> Result<B, Error> {
    let aad = header.aad(record_header.route);
    let nonce = Nonce::from_counter(header.seq.0);
    let mut ciphertext = encrypted.ciphertext;
    if !crypto.aes256_gcm_decrypt(
        session_key,
        &nonce,
        &aad,
        ciphertext.as_mut(),
        &encrypted.auth,
    ) {
        return Err(Error::DecryptFailed);
    }
    Ok(ciphertext)
}
