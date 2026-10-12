use std::{
    collections::BTreeMap,
    fmt::Debug,
    num::NonZeroU8,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ql_codec::{Codec, Decode, Encode, Reader, Ref};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Codec)]
#[codec(frozen)]
pub struct AppId(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Codec)]
pub enum Location {
    System,
    AppData,
    Usb,
}

#[derive(Debug, Clone, PartialEq, Eq, Codec)]
#[codec(derive(Debug, PartialEq))]
pub enum Permission {
    Camera,
    Files { location: Location, read_only: bool },
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Codec)]
pub struct InstalledApp {
    pub app_id: AppId,
    pub pid: Option<NonZeroU8>,
    pub name: String,
    pub icon: Vec<u8>,
    pub permissions: Vec<Permission>,
    pub signatures: Vec<Vec<u8>>,
    pub keywords: Vec<String>,
    pub nine_slice: Option<[u16; 4]>,
    pub size: usize,
    pub installed_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq, Codec)]
pub enum AppManagerError {
    NotInstalled(AppId),
    PermissionDenied {
        location: Location,
    },
    Io(String),
    #[codec(unknown)]
    InternalError,
}

#[derive(Debug, Clone, PartialEq, Eq, Codec)]
pub struct WriteFile {
    pub path: String,
    pub data: Vec<u8>,
    pub location: Location,
}

#[derive(Debug, Clone, PartialEq, Eq, Codec)]
pub struct ListApps;

#[derive(Debug, Clone, PartialEq, Eq, Codec)]
pub struct Transceive(pub (Vec<u8>, Vec<u8>, Duration));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Codec)]
#[codec(discriminants = SeekFromKind)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

#[derive(Debug, Clone, PartialEq, Eq, Codec)]
pub struct IconSet(pub BTreeMap<String, Vec<u8>>);

#[derive(Debug, Clone, PartialEq, Eq, Codec)]
#[codec(frozen)]
pub struct Envelope<B> {
    pub cid: u32,
    pub message: B,
}

#[derive(Debug, Clone, PartialEq, Eq, Codec)]
pub enum Reply<T, E> {
    Done(T),
    Failed(Vec<E>),
    #[codec(unknown)]
    Unknown,
}

/// A module with its own `Result`, which the derive's output must not pick up
#[allow(dead_code)]
mod own_result {
    use ql_codec::Codec;

    pub type Result<T> = std::result::Result<T, String>;

    #[derive(Codec)]
    pub struct Request {
        pub id: u32,
    }

    #[derive(Codec)]
    pub enum Response {
        Done(u32),
        #[codec(unknown)]
        Unknown,
    }
}

fn installed_app() -> InstalledApp {
    InstalledApp {
        app_id: AppId(*b"wallet-app-id-16"),
        pid: NonZeroU8::new(42),
        name: "Wallet".into(),
        icon: (0..=255).collect(),
        permissions: vec![
            Permission::Camera,
            Permission::Files {
                location: Location::AppData,
                read_only: true,
            },
            Permission::Custom("dev".into()),
        ],
        signatures: vec![vec![0xaa; 64], vec![]],
        keywords: vec!["bitcoin".into(), "".into()],
        nine_slice: Some([4, 4, 8, 8]),
        size: 3 << 20,
        installed_at: UNIX_EPOCH + Duration::new(1_700_000_000, 999_999_999),
    }
}

fn roundtrip<T>(value: &T)
where
    T: Encode + Debug + PartialEq,
    for<'a> T: Decode<&'a [u8]> + Decode<&'a mut [u8]>,
{
    let mut bytes = value.encode_vec();
    assert_eq!(T::decode_bytes(bytes.as_slice()).unwrap(), *value);
    let mut reader = Reader::new(bytes.as_mut_slice());
    assert_eq!(T::decode(&mut reader).unwrap(), *value);
    assert!(reader.is_empty());
}

#[test]
fn borrowed_form_points_into_the_encoding() {
    let app = installed_app();
    let bytes = app.encode_vec();
    let decoded = InstalledApp::decode_bytes_ref(bytes.as_slice()).unwrap();

    let within = |slice: &[u8]| bytes.as_ptr_range().contains(&slice.as_ptr());
    let app_id: &[u8; 16] = &decoded.app_id.0;
    let icon: &[u8] = &decoded.icon;
    let name: &str = &decoded.name;
    let signatures: &[Ref<&[u8], [u8]>] = &decoded.signatures;
    let keywords: &[Ref<&[u8], str>] = &decoded.keywords;
    assert!(within(app_id));
    assert!(within(icon));
    assert!(within(name.as_bytes()));
    assert!(within(&signatures[0]));
    assert!(within(keywords[0].as_bytes()));
    assert_eq!(
        decoded.permissions[2],
        PermissionRef::Custom(Ref::new(&b"dev"[..]).unwrap())
    );

    assert_eq!(decoded.encode_vec(), bytes);
    assert_eq!(InstalledApp::from(decoded), app);
}

#[test]
fn borrowed_form_encodes_like_the_owned_one() {
    let data = [0x5a; 300];
    let write = WriteFileRef {
        path: Ref::new(&b"notes.txt"[..]).unwrap(),
        data: Ref::new(data.as_slice()).unwrap(),
        location: Location::AppData,
    };
    let bytes = write.encode_vec();

    let owned = WriteFile {
        path: "notes.txt".into(),
        data: data.to_vec(),
        location: Location::AppData,
    };
    assert_eq!(bytes, owned.encode_vec());
    assert_eq!(WriteFile::decode_bytes(bytes.as_slice()).unwrap(), owned);
}

#[test]
fn responses_roundtrip() {
    let sparse = InstalledApp {
        pid: None,
        icon: vec![],
        permissions: vec![],
        nine_slice: None,
        ..installed_app()
    };
    let responses: [Result<Vec<InstalledApp>, AppManagerError>; 6] = [
        Ok(vec![]),
        Ok(vec![installed_app(), sparse]),
        Err(AppManagerError::NotInstalled(AppId([7; 16]))),
        Err(AppManagerError::PermissionDenied {
            location: Location::Usb,
        }),
        Err(AppManagerError::Io("disk full".into())),
        Err(AppManagerError::InternalError),
    ];
    for response in &responses {
        roundtrip(response);
    }
}

#[test]
fn maps_roundtrip() {
    let icons = IconSet(BTreeMap::from([
        ("wallet".to_string(), vec![1, 2, 3]),
        ("camera".to_string(), vec![]),
    ]));
    roundtrip(&icons);
    roundtrip(&IconSet(BTreeMap::new()));

    let bytes = icons.encode_vec();
    let borrowed = IconSet::decode_bytes_ref(bytes.as_slice()).unwrap();
    assert_eq!(*borrowed.0["wallet"], [1, 2, 3]);
    assert_eq!(borrowed.encode_vec(), bytes);
}

#[test]
fn generic_types_roundtrip() {
    let envelope = Envelope {
        cid: 7,
        message: WriteFile {
            path: "notes.txt".into(),
            data: vec![1, 2, 3],
            location: Location::Usb,
        },
    };
    roundtrip(&envelope);
    let bytes = envelope.encode_vec();
    let borrowed = Envelope::<WriteFile>::decode_bytes_ref(bytes.as_slice()).unwrap();
    assert_eq!(borrowed.message.path, "notes.txt");
    assert_eq!(borrowed.encode_vec(), bytes);
    assert_eq!(borrowed.into_owned(), envelope);

    for reply in [
        Reply::Done(3u64),
        Reply::Failed(vec!["disk full".to_string()]),
        Reply::Unknown,
    ] {
        roundtrip(&reply);
    }
}

#[test]
fn messages_roundtrip() {
    roundtrip(&ListApps);
    roundtrip(&Transceive((
        vec![0x00, 0xa4, 0x04, 0x00],
        vec![0x90, 0x00],
        Duration::from_millis(250),
    )));
    for seek in [
        SeekFrom::Start(u64::MAX),
        SeekFrom::End(-17),
        SeekFrom::Current(i64::MIN),
    ] {
        roundtrip(&seek);
        assert_eq!(seek.encode_vec()[0], seek.discriminant() as u8);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Codec)]
pub struct Samples {
    pub name: String,
    pub values: Vec<u32>,
    pub sid: [u64; 2],
}

fn samples(name: &str) -> Samples {
    Samples {
        name: name.into(),
        values: vec![1, u32::MAX, 3],
        sid: [u64::MAX, 5],
    }
}

#[repr(align(8))]
struct AlignedBuffer([u8; 256]);

/// `bytes` copied into a buffer aligned for any integer, `offset` bytes in
fn copy_aligned(bytes: &[u8], offset: usize) -> AlignedBuffer {
    let mut buffer = AlignedBuffer([0; 256]);
    buffer.0[offset..][..bytes.len()].copy_from_slice(bytes);
    buffer
}

#[test]
fn integer_lists_are_lent_aligned() {
    // Each name shifts the integers after it differently.
    let list = Vec::from(["", "a", "ab", "abc"].map(samples));
    let bytes = list.encode_vec();
    let buffer = copy_aligned(&bytes, 0);
    let encoding = &buffer.0[..bytes.len()];
    let decoded = Vec::<Samples>::decode_bytes_ref(encoding).unwrap();

    for (decoded, samples) in decoded.iter().zip(&list) {
        let values: &[u32] = &decoded.values;
        let sid: &[u64; 2] = &decoded.sid;
        assert_eq!(values, samples.values);
        assert_eq!(sid, &samples.sid);
        assert!(encoding.as_ptr_range().contains(&values.as_ptr().cast()));
        assert!(encoding.as_ptr_range().contains(&sid.as_ptr().cast()));
    }
    assert_eq!(decoded.encode_vec(), bytes);
}

#[test]
fn mutable_integer_lists_change_the_encoding_in_place() {
    let mut expected = samples("ab");
    let bytes = expected.encode_vec();
    let mut buffer = copy_aligned(&bytes, 0);
    let encoding = &mut buffer.0[..bytes.len()];
    let mut view = Samples::decode_bytes_ref(&mut *encoding).unwrap();
    view.values.reverse();
    view.sid[1] = 9;

    expected.values.reverse();
    expected.sid[1] = 9;
    assert_eq!(Samples::decode_bytes(&*encoding).unwrap(), expected);
}

#[test]
fn misaligned_integer_lists_are_only_copied() {
    let samples = samples("ab");
    let bytes = samples.encode_vec();
    let buffer = copy_aligned(&bytes, 1);
    let encoding = &buffer.0[1..][..bytes.len()];
    assert!(matches!(
        Samples::decode_bytes_ref(encoding),
        Err(ql_codec::Error::InvalidAlignment)
    ));
    assert_eq!(Samples::decode_bytes(encoding).unwrap(), samples);
}
