use bytes::Buf;
use ql_codec::{encode_bytes_raw, BufView, ByteSlice, Decode, Encode, Reader};

use crate::ENCRYPTED_MESSAGE_AUTH_SIZE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedMessage<B> {
    pub auth: [u8; ENCRYPTED_MESSAGE_AUTH_SIZE],
    pub ciphertext: B,
}

impl<B: AsRef<[u8]>> EncryptedMessage<B> {
    pub fn into_owned(self) -> EncryptedMessage<Vec<u8>> {
        EncryptedMessage {
            auth: self.auth,
            ciphertext: self.ciphertext.as_ref().to_vec(),
        }
    }
}

impl<B: ByteSlice> Decode<B> for EncryptedMessage<Vec<u8>> {
    type Ref = EncryptedMessage<B>;

    fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, ql_codec::Error> {
        Ok(EncryptedMessage {
            auth: reader.decode()?,
            ciphertext: reader.take_all(),
        })
    }

    fn from_ref(value: Self::Ref) -> Self {
        Self {
            auth: value.auth,
            ciphertext: value.ciphertext.to_vec(),
        }
    }
}

impl<B: BufView> Encode for EncryptedMessage<B> {
    fn encoded_len(&self) -> usize {
        self.auth.encoded_len() + self.ciphertext.buf().remaining()
    }

    fn encode<W: ::bytes::BufMut + ?Sized>(&self, out: &mut W) {
        self.auth.encode(out);
        encode_bytes_raw(&self.ciphertext, out);
    }
}
