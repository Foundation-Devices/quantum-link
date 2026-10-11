use ql_codec::{Aligned, Codec, Decode, Encode, Error, Reader};

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
    let numbers: &[u32] = &borrowed.numbers;
    let pair: &[i16; 2] = borrowed.pair.as_ref();
    assert_eq!(numbers, message.numbers.0);
    assert_eq!(pair, &message.pair.0);
    assert_eq!(message.numbers.encode_vec(), message.numbers.0.encode_vec());
    assert_eq!(message.pair.encode_vec(), message.pair.0.encode_vec());
    assert_eq!(numbers.as_ptr().cast::<u8>(), bytes[4..].as_ptr());
    assert_eq!(pair.as_ptr().cast::<u8>(), bytes[12..].as_ptr());
    assert_eq!(borrowed.encode_vec(), bytes);
    assert_eq!(borrowed.into_owned(), message);

    let mut reader = Reader::new(&mut buffer.0[..message.encoded_len()]);
    let mut borrowed = reader.decode_ref::<Message>().unwrap();
    assert!(reader.is_empty());
    assert_eq!(borrowed.numbers.as_ref(), message.numbers.0);
    assert_eq!(borrowed.pair.as_slice(), &message.pair.0);
    borrowed.numbers[1] = 42;
    borrowed.pair[0] = 3;
    let changed = Message {
        padding: [0; 2],
        numbers: Aligned(vec![1, 42]),
        pair: Aligned([3, i16::MIN]),
    };
    assert_eq!(borrowed.into_owned(), changed);
    assert_eq!(&buffer.0[..message.encoded_len()], changed.encode_vec());
    assert_eq!(
        Message::decode_bytes_ref(buffer.0.as_slice())
            .unwrap()
            .into_owned(),
        changed
    );

    let mut shifted = Buffer([0; 64]);
    message.encode(&mut &mut shifted.0[1..]);
    let bytes = &shifted.0[1..1 + message.encoded_len()];
    assert!(matches!(
        Message::decode_bytes_ref(bytes),
        Err(Error::InvalidAlignment)
    ));
    assert_eq!(Message::decode_bytes(bytes).unwrap(), message);
    assert!(matches!(
        Reader::new(&mut shifted.0[1..1 + message.encoded_len()]).decode_ref::<Message>(),
        Err(Error::InvalidAlignment)
    ));
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
    assert_eq!(values.as_ref(), &[7, 9]);
    assert_eq!(values.as_ptr().cast::<u8>(), bytes[4..].as_ptr());
    assert_eq!(borrowed.encode_vec(), bytes);
    let Reply::Values { values, .. } = borrowed.into_owned();
    assert_eq!(values.0, [7, 9]);

    let mut reader = Reader::new(&mut buffer.0[..reply.encoded_len()]);
    let ReplyRef::Values { mut values, .. } =
        reader.decode_ref::<Reply<Aligned<Vec<u32>>>>().unwrap();
    values[0] = 11;
    let ReplyRef::Values { values, .. } =
        Reply::<Aligned<Vec<u32>>>::decode_bytes_ref(buffer.0.as_slice()).unwrap();
    assert_eq!(values.as_ref(), &[11, 9]);
}

#[test]
fn empty_and_missing_aligned_fields() {
    let missing = Message::decode_bytes_ref(&[0][..]).unwrap();
    assert!(missing.numbers.is_empty());
    assert_eq!(missing.pair.as_slice(), &[0, 0]);
    let defaults = <Message as Decode<&[u8]>>::missing_ref().unwrap();
    assert!(defaults.numbers.is_empty());
    assert_eq!(defaults.pair.as_slice(), &[0, 0]);
    assert!(Aligned::<Vec<u32>>::decode_bytes_ref(&[0][..])
        .unwrap()
        .is_empty());
    assert!(Aligned::<[u32; 0]>::decode_bytes_ref(&[][..])
        .unwrap()
        .is_empty());
}

#[test]
#[cfg(target_endian = "little")]
fn truncated_aligned_payloads_fail() {
    assert_eq!(
        Aligned::<Vec<u32>>::decode_bytes_ref(&[2, 1, 0, 0, 0][..]),
        Err(Error::UnexpectedEof)
    );
    assert_eq!(
        Aligned::<[u32; 2]>::decode_bytes_ref(&[0; 4][..]),
        Err(Error::UnexpectedEof)
    );
    let error =
        Aligned::<Vec<u64>>::decode_bytes_ref(&[0xff, 0xff, 0xff, 0xff, 0x0f][..]).unwrap_err();
    assert!(matches!(
        error,
        Error::UnexpectedEof | Error::LengthOverflow
    ));
    let mut bytes = [2, 1, 0, 0, 0];
    assert!(matches!(
        Reader::new(bytes.as_mut_slice()).decode_ref::<Aligned<Vec<u32>>>(),
        Err(Error::UnexpectedEof)
    ));
}

#[test]
fn byte_collections_borrow_only_when_wrapped() {
    let bytes = [3, 7, 9, 11];
    let list: Vec<u8> = Vec::<u8>::decode_bytes_ref(bytes.as_slice()).unwrap();
    let array: [u8; 3] = <[u8; 3]>::decode_bytes_ref(&bytes[1..]).unwrap();
    let borrowed = Aligned::<Vec<u8>>::decode_bytes_ref(bytes.as_slice()).unwrap();
    let borrowed_array = Aligned::<[u8; 3]>::decode_bytes_ref(&bytes[1..]).unwrap();
    assert_eq!(list, array);
    assert_eq!(borrowed.as_ref(), list);
    assert_eq!(borrowed_array.as_slice(), &array);
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
        fn decode_value<B: ql_codec::ByteSlice>(
            reader: &mut ql_codec::Reader<B>,
        ) -> Result<Self, Error> {
            reader.take_u8().map(Self)
        }
    }

    let bytes = [2, 7, 9];
    let list = Vec::<Number>::decode_bytes_ref(bytes.as_slice()).unwrap();
    let array = <[Number; 2]>::decode_bytes_ref(&bytes[1..]).unwrap();
    assert_eq!(list, [Number(7), Number(9)]);
    assert_eq!(array, [Number(7), Number(9)]);
    assert_eq!(list.encode_vec(), bytes);
    assert_eq!(array.encode_vec(), bytes[1..]);
}
