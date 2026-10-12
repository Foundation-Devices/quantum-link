use std::{
    collections::{hash_map::RandomState, BTreeMap},
    hash::BuildHasher,
};

use ql_codec::{Codec, Decode, Encode, Error, Reader, Ref};

#[derive(Debug, PartialEq, Codec)]
enum Item<B> {
    Text(B),
    #[codec(unknown)]
    Unknown,
}

#[derive(Debug, PartialEq, Codec)]
struct Message {
    items: Vec<Option<Item<String>>>,
    outcome: Result<Box<[u8; 2]>, String>,
    attributes: BTreeMap<String, String>,
    pair: (Box<[u8]>, String),
    count: u32,
}

#[test]
fn nested_views_mutate_the_encoding_and_outlive_the_reader() {
    let mut expected = Message {
        items: vec![Some(Item::Text("hello".into())), None],
        outcome: Ok(Box::new([1, 2])),
        attributes: BTreeMap::from([("key".into(), "value".into())]),
        pair: (Box::from(&b"bytes"[..]), "tail".into()),
        count: 42,
    };
    let mut bytes = expected.encode_vec();
    bytes.push(9);
    let (mut view, tail) = {
        let mut reader = Reader::new(bytes.as_mut_slice());
        let view: MessageRef<&mut [u8]> = reader.decode_ref::<Message>().unwrap();
        let tail = reader.take_all();
        (view, tail)
    };
    let Some(ItemRef::Text(text)) = &mut view.items[0] else {
        panic!("expected text");
    };
    text.make_ascii_uppercase();
    let fixed = view.outcome.as_mut().unwrap();
    AsMut::<[u8]>::as_mut(fixed)[0] = 3;
    assert_eq!(AsRef::<[u8]>::as_ref(fixed), &[3, 2]);
    let (mut key, mut value) = view.attributes.pop_first().unwrap();
    key.make_ascii_uppercase();
    value.make_ascii_uppercase();
    view.attributes.insert(key, value);
    view.pair.0[0] = b'B';
    view.pair.1.make_ascii_uppercase();
    tail[0] = 10;

    expected.items[0] = Some(Item::Text("HELLO".into()));
    expected.outcome = Ok(Box::new([3, 2]));
    expected.attributes = BTreeMap::from([("KEY".into(), "VALUE".into())]);
    expected.pair = (Box::from(&b"Bytes"[..]), "TAIL".into());
    assert_eq!(view.encode_vec(), expected.encode_vec());
    assert_eq!(view.into_owned(), expected);
    assert_eq!(bytes.last(), Some(&10));
    let shared: MessageRef<&[u8]> = Reader::new(bytes.as_slice())
        .decode_ref::<Message>()
        .unwrap();
    assert_eq!(shared.into_owned(), expected);
}

#[test]
fn missing_mutable_fields_only_default_without_backing_storage() {
    #[derive(Codec)]
    struct Defaults {
        text: String,
        bytes: Box<[u8]>,
        items: Vec<String>,
        fixed: Option<[u32; 2]>,
        empty: Box<[u8; 0]>,
    }

    let mut bytes = [0];
    let view = Reader::new(bytes.as_mut_slice())
        .decode_ref::<Defaults>()
        .unwrap();
    assert_eq!(view.text, "");
    assert!(view.bytes.is_empty());
    assert!(view.items.is_empty());
    assert!(view.fixed.is_none());
    assert!(view.empty.is_empty());

    #[derive(Codec)]
    struct Fixed {
        bytes: Box<[u8; 2]>,
    }

    assert_eq!(Fixed::decode_bytes_ref(&[0][..]).unwrap().bytes, &[0, 0]);
    assert!(matches!(
        Reader::new(bytes.as_mut_slice()).decode_ref::<Fixed>(),
        Err(Error::UnexpectedEof)
    ));
    assert_eq!(
        *Fixed::decode_bytes(bytes.as_mut_slice()).unwrap().bytes,
        [0, 0]
    );
    assert!(<[u32; 2] as Decode<&mut [u8]>>::missing_ref().is_none());
    assert!(<[u32; 0] as Decode<&mut [u8]>>::missing_ref()
        .unwrap()
        .is_empty());

    #[derive(Codec)]
    struct Integers {
        words: [u32; 2],
        samples: Vec<u16>,
    }

    let integers = Integers::decode_bytes_ref(&[0][..]).unwrap();
    assert_eq!(*integers.words, [0, 0]);
    assert!(integers.samples.is_empty());
}

#[test]
fn mutable_strings_validate_utf8_and_length() {
    let mut invalid = [1, 0xff];
    assert!(matches!(
        String::decode_bytes_ref(invalid.as_mut_slice()),
        Err(Error::InvalidUtf8)
    ));
    let mut truncated = [2, b'a'];
    assert!(matches!(
        String::decode_bytes(truncated.as_mut_slice()),
        Err(Error::UnexpectedEof)
    ));
}

#[test]
fn shared_views_derive_value_traits_and_fixed_refs_validate_length() {
    #[derive(Codec)]
    #[codec(frozen, derive(Debug, Clone, Copy, PartialEq, Eq, Hash))]
    struct Header {
        bytes: [u8; 2],
        text: String,
    }

    let bytes = Header {
        bytes: [1, 2],
        text: "hello".into(),
    }
    .encode_vec();
    let shared = Header::decode_bytes_ref(bytes.as_slice()).unwrap();
    let copied = shared;
    assert_eq!(copied, shared);
    let fixed: &[u8] = copied.bytes.as_ref();
    assert_eq!(fixed, &[1, 2]);
    assert_eq!(fixed.as_ptr(), bytes.as_ptr());
    let hasher = RandomState::new();
    assert_eq!(
        hasher.hash_one(shared),
        hasher.hash_one(([1u8, 2], "hello"))
    );
    assert_eq!(shared.into_owned().bytes, [1, 2]);
    assert!(matches!(
        Ref::<_, [u8; 2]>::new(&bytes[..1]),
        Err(Error::InvalidData)
    ));
}
