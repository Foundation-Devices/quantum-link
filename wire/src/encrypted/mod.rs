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
pub enum SessionFrame {
    // todo: do we need ping as explicit frame?
    Ping = 1,
    Ack(RecordAck) = 2,
    StreamData(StreamData<Vec<u8>>) = 3,
    StreamWindow(StreamWindow) = 4,
    StreamReset(StreamReset) = 5,
    Close(SessionClose) = 6,
    Unpair = 7,
}

pub fn parse_session_frames(bytes: &[u8]) -> SessionFrameIter<'_> {
    SessionFrameIter {
        reader: Reader::new(bytes),
    }
}

pub fn decode_session_frames(bytes: &[u8]) -> Result<Vec<SessionFrame>, Error> {
    parse_session_frames(bytes)
        .map(|frame| frame.map(SessionFrameRef::into_owned))
        .collect()
}

#[derive(Clone)]
pub struct SessionFrameIter<'a> {
    reader: Reader<'a>,
}

impl<'a> Iterator for SessionFrameIter<'a> {
    type Item = Result<SessionFrameRef<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.reader.is_empty() {
            None
        } else {
            Some(self.reader.decode_ref::<SessionFrame>().map_err(Into::into))
        }
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
