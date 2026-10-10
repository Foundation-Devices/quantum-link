use bytes::Buf;
use ql_codec::{encode_bytes_raw, BufView, Decode, Encode, Reader};

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

impl Decode for EncryptedMessage<Vec<u8>> {
    type Ref<'a> = EncryptedMessage<&'a [u8]>;

    fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, ql_codec::Error> {
        Ok(EncryptedMessage {
            auth: reader.decode()?,
            ciphertext: reader.take_all(),
        })
    }

    fn from_ref(value: Self::Ref<'_>) -> Self {
        value.into_owned()
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
