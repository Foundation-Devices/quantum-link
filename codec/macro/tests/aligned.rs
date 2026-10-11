use ql_codec::{Aligned, Codec, Decode, Encode, Error};

#[derive(Debug, PartialEq, Codec)]
#[codec(derive(Debug))]
struct Message {
    padding: [u8; 2],
    numbers: Aligned<Vec<u32>>,
    pair: Aligned<[i16; 2]>,
}

#[repr(align(8))]
struct Buffer([u8; 64]);

#[test]
#[cfg(target_endian = "little")]
fn aligned_fields_borrow_the_encoding() {
    let message = Message {
        padding: [0; 2],
        numbers: Aligned(vec![1, u32::MAX]),
        pair: Aligned([-1, i16::MIN]),
    };
    let mut buffer = Buffer([0; 64]);
    message.encode(&mut buffer.0.as_mut_slice());
    let bytes = &buffer.0[..message.encoded_len()];

    let borrowed = Message::decode_bytes_ref(bytes).unwrap();
    let numbers: &[u32] = borrowed.numbers;
    let pair: &[i16; 2] = borrowed.pair;
    assert_eq!(numbers, message.numbers.0);
    assert_eq!(pair, &message.pair.0);
    assert_eq!(message.numbers.encode_vec(), message.numbers.0.encode_vec());
    assert_eq!(message.pair.encode_vec(), message.pair.0.encode_vec());
    assert_eq!(numbers.as_ptr().cast::<u8>(), bytes[4..].as_ptr());
    assert_eq!(pair.as_ptr().cast::<u8>(), bytes[12..].as_ptr());
    assert_eq!(borrowed.encode_vec(), bytes);
    assert_eq!(borrowed.into_owned(), message);

    let mut shifted = Buffer([0; 64]);
    message.encode(&mut &mut shifted.0[1..]);
    let bytes = &shifted.0[1..1 + message.encoded_len()];
    assert!(matches!(
        Message::decode_bytes_ref(bytes),
        Err(Error::InvalidAlignment)
    ));
    assert_eq!(Message::decode_bytes(bytes).unwrap(), message);
}

#[test]
#[cfg(target_endian = "little")]
fn frozen_generic_enum_borrows_aligned_fields() {
    #[derive(Codec)]
    #[codec(frozen)]
    enum Reply<T> {
        Values { padding: [u8; 2], values: T },
    }

    let reply = Reply::Values {
        padding: [0; 2],
        values: Aligned(vec![7u32, 9]),
    };
    let mut buffer = Buffer([0; 64]);
    reply.encode(&mut buffer.0.as_mut_slice());
    let bytes = &buffer.0[..reply.encoded_len()];
    let borrowed = Reply::<Aligned<Vec<u32>>>::decode_bytes_ref(bytes).unwrap();
    let ReplyRef::Values { values, .. } = &borrowed;
    assert_eq!(*values, &[7, 9]);
    assert_eq!(values.as_ptr().cast::<u8>(), bytes[4..].as_ptr());
    assert_eq!(borrowed.encode_vec(), bytes);
    let Reply::Values { values, .. } = borrowed.into_owned();
    assert_eq!(values.0, [7, 9]);
}

#[test]
fn empty_and_missing_aligned_fields() {
    let missing = Message::decode_bytes_ref(&[0]).unwrap();
    assert_eq!(missing.numbers, &[]);
    assert_eq!(missing.pair, &[0, 0]);
    let defaults = Message::missing().unwrap();
    assert_eq!(defaults.numbers, &[]);
    assert_eq!(defaults.pair, &[0, 0]);
    assert_eq!(Aligned::<Vec<u32>>::decode_bytes_ref(&[0]).unwrap(), &[]);
    assert_eq!(Aligned::<[u32; 0]>::decode_bytes_ref(&[]).unwrap(), &[]);
}

#[test]
#[cfg(target_endian = "little")]
fn truncated_aligned_payloads_fail() {
    assert_eq!(
        Aligned::<Vec<u32>>::decode_bytes_ref(&[2, 1, 0, 0, 0]),
        Err(Error::UnexpectedEof)
    );
    assert_eq!(
        Aligned::<[u32; 2]>::decode_bytes_ref(&[0; 4]),
        Err(Error::UnexpectedEof)
    );
    let error = Aligned::<Vec<u64>>::decode_bytes_ref(&[0xff, 0xff, 0xff, 0xff, 0x0f]).unwrap_err();
    assert!(matches!(
        error,
        Error::UnexpectedEof | Error::LengthOverflow
    ));
}

#[test]
fn byte_collections_borrow_only_when_wrapped() {
    let bytes = [3, 7, 9, 11];
    let list: Vec<u8> = Vec::<u8>::decode_bytes_ref(&bytes).unwrap();
    let array: [u8; 3] = <[u8; 3]>::decode_bytes_ref(&bytes[1..]).unwrap();
    let borrowed: &[u8] = Aligned::<Vec<u8>>::decode_bytes_ref(&bytes).unwrap();
    let borrowed_array: &[u8; 3] = Aligned::<[u8; 3]>::decode_bytes_ref(&bytes[1..]).unwrap();
    assert_eq!(list, array);
    assert_eq!(borrowed, list);
    assert_eq!(borrowed_array, &array);
    assert_eq!(borrowed.as_ptr(), bytes[1..].as_ptr());
    assert_eq!(borrowed_array.as_ptr(), bytes[1..].as_ptr());
    assert_eq!(list.encode_vec(), bytes);
    assert_eq!(array.encode_vec(), bytes[1..]);
    assert_eq!(borrowed.encode_vec(), bytes);
    assert_eq!(borrowed_array.encode_vec(), bytes[1..]);
}

#[test]
fn hand_written_codecs_work_in_collections() {
    #[derive(Debug, PartialEq)]
    struct Number(u8);

    impl Encode for Number {
        fn encoded_len(&self) -> usize {
            self.0.encoded_len()
        }

        fn encode<W: ql_codec::BufMut + ?Sized>(&self, out: &mut W) {
            self.0.encode(out);
        }
    }

    impl ql_codec::DecodeValue for Number {
        fn decode_value(reader: &mut ql_codec::Reader<'_>) -> Result<Self, Error> {
            reader.take_u8().map(Self)
        }
    }

    let bytes = [2, 7, 9];
    let list = Vec::<Number>::decode_bytes_ref(&bytes).unwrap();
    let array = <[Number; 2]>::decode_bytes_ref(&bytes[1..]).unwrap();
    assert_eq!(list, [Number(7), Number(9)]);
    assert_eq!(array, [Number(7), Number(9)]);
    assert_eq!(list.encode_vec(), bytes);
    assert_eq!(array.encode_vec(), bytes[1..]);
}
