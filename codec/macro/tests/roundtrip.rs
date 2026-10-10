use std::{
    fmt::Debug,
    num::NonZeroU8,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ql_codec::{Codec, Decode, Encode};

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

fn roundtrip<T: Encode + Decode + Debug + PartialEq>(value: &T) {
    let bytes = value.encode_vec();
    assert_eq!(T::decode_bytes(&bytes).unwrap(), *value);
}

#[test]
fn borrowed_form_points_into_the_encoding() {
    let app = installed_app();
    let bytes = app.encode_vec();
    let decoded = InstalledApp::decode_bytes_ref(&bytes).unwrap();

    let within = |slice: &[u8]| bytes.as_ptr_range().contains(&slice.as_ptr());
    let app_id: &[u8; 16] = decoded.app_id.0;
    let icon: &[u8] = decoded.icon;
    let name: &str = decoded.name;
    let signatures: &[&[u8]] = &decoded.signatures;
    let keywords: &[&str] = &decoded.keywords;
    assert!(within(app_id));
    assert!(within(icon));
    assert!(within(name.as_bytes()));
    assert!(within(signatures[0]));
    assert!(within(keywords[0].as_bytes()));
    assert_eq!(decoded.permissions[2], PermissionRef::Custom("dev"));

    assert_eq!(decoded.encode_vec(), bytes);
    assert_eq!(InstalledApp::from(decoded), app);
}

#[test]
fn borrowed_form_encodes_like_the_owned_one() {
    let data = [0x5a; 300];
    let write = WriteFileRef {
        path: "notes.txt",
        data: &data,
        location: Location::AppData,
    };
    let bytes = write.encode_vec();

    let owned = WriteFile {
        path: "notes.txt".into(),
        data: data.to_vec(),
        location: Location::AppData,
    };
    assert_eq!(bytes, owned.encode_vec());
    assert_eq!(WriteFile::decode_bytes(&bytes).unwrap(), owned);
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
