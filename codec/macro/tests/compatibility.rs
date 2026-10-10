use std::{fmt::Debug, num::NonZeroU8};

use ql_codec::{Codec, Decode, Encode, Error};

mod v1 {
    use ql_codec::Codec;

    #[derive(Debug, PartialEq, Eq, Codec)]
    pub struct DeviceInfo {
        pub name: String,
        pub battery: u8,
    }

    #[derive(Debug, PartialEq, Eq, Codec)]
    pub enum UsbEvent {
        Connected,
        Disconnected,
        #[codec(unknown)]
        Unknown,
    }

    #[derive(Debug, PartialEq, Eq, Codec)]
    pub struct Status {
        pub device: DeviceInfo,
        pub events: Vec<UsbEvent>,
        pub uptime: u32,
    }
}

/// `v1` grown in the compatible ways: fields and variants appended, and a unit variant gaining
/// fields
mod v2 {
    use ql_codec::Codec;

    #[derive(Debug, PartialEq, Eq, Codec)]
    pub struct DeviceInfo {
        pub name: String,
        pub battery: u8,
        pub serial: Option<String>,
        pub charging: bool,
    }

    #[derive(Debug, PartialEq, Eq, Codec)]
    pub enum UsbEvent {
        Connected {
            speed: u32,
        },
        Disconnected,
        #[codec(unknown)]
        Unknown,
        Suspended,
    }

    #[derive(Debug, PartialEq, Eq, Codec)]
    pub struct Status {
        pub device: DeviceInfo,
        pub events: Vec<UsbEvent>,
        pub uptime: u32,
        pub tags: Vec<String>,
    }
}

#[derive(Debug, PartialEq, Eq, Codec)]
#[codec(frozen)]
pub struct ImageSize {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, PartialEq, Eq, Codec)]
#[codec(frozen)]
pub enum PixelFormat {
    Rgb = 1,
    Rgba,
    AlphaMap = 7,
}

#[derive(Debug, PartialEq, Eq, Codec)]
#[codec(frozen)]
pub enum Shape {
    Point,
    Image(ImageSize, PixelFormat),
}

#[derive(Debug, PartialEq, Eq, Codec)]
pub enum ChargeState {
    Idle,
    Charging,
}

#[derive(Debug, PartialEq, Eq, Codec)]
pub struct Process {
    pub pid: NonZeroU8,
}

#[rustfmt::skip]
const V1_STATUS: &[u8] = &[
    14, // length
    4, 2, b'k', b'b', 80, // device
    2, 0, 0, 1, 0, // events
    0x02, 0x01, 0x00, 0x00, // uptime
];

#[rustfmt::skip]
const V2_STATUS: &[u8] = &[
    28, // length
    9, 2, b'k', b'b', 80, 1, 2, b'A', b'1', 1, // device
    3, 0, 4, 0xe0, 0x01, 0x00, 0x00, 3, 0, 1, 0, // events
    0x02, 0x01, 0x00, 0x00, // uptime
    1, 1, b'x', // tags
];

const IMAGE_SHAPE: &[u8] = &[1, 0x40, 0x01, 0xf0, 0x00, 7];

fn v1_status() -> v1::Status {
    v1::Status {
        device: v1::DeviceInfo {
            name: "kb".into(),
            battery: 80,
        },
        events: vec![v1::UsbEvent::Connected, v1::UsbEvent::Disconnected],
        uptime: 0x0102,
    }
}

fn v2_status() -> v2::Status {
    v2::Status {
        device: v2::DeviceInfo {
            name: "kb".into(),
            battery: 80,
            serial: Some("A1".into()),
            charging: true,
        },
        events: vec![
            v2::UsbEvent::Connected { speed: 480 },
            v2::UsbEvent::Suspended,
            v2::UsbEvent::Disconnected,
        ],
        uptime: 0x0102,
        tags: vec!["x".into()],
    }
}

fn image_shape() -> Shape {
    Shape::Image(
        ImageSize {
            width: 320,
            height: 240,
        },
        PixelFormat::AlphaMap,
    )
}

/// Decodes both forms, which the derive generates separately
fn decode<T: Decode + Debug + PartialEq>(bytes: &[u8]) -> T {
    let owned = T::decode_bytes(bytes).unwrap();
    assert_eq!(T::from_ref(T::decode_bytes_ref(bytes).unwrap()), owned);
    owned
}

#[test]
fn encodings_match_fixed_bytes() {
    assert_eq!(v1_status().encode_vec(), V1_STATUS);
    assert_eq!(v2_status().encode_vec(), V2_STATUS);
    assert_eq!(image_shape().encode_vec(), IMAGE_SHAPE);
    assert_eq!(PixelFormat::Rgba.encode_vec(), [2]);

    assert_eq!(decode::<v1::Status>(V1_STATUS), v1_status());
    assert_eq!(decode::<v2::Status>(V2_STATUS), v2_status());
    assert_eq!(decode::<Shape>(IMAGE_SHAPE), image_shape());
}

#[test]
fn new_decoder_fills_missing_fields() {
    let status: v2::Status = decode(V1_STATUS);
    assert_eq!(
        status,
        v2::Status {
            device: v2::DeviceInfo {
                name: "kb".into(),
                battery: 80,
                serial: None,
                charging: false,
            },
            events: vec![
                v2::UsbEvent::Connected { speed: 0 },
                v2::UsbEvent::Disconnected
            ],
            uptime: 0x0102,
            tags: vec![],
        }
    );
}

#[test]
fn old_decoder_skips_new_fields_and_variants() {
    let status: v1::Status = decode(V2_STATUS);
    assert_eq!(
        status,
        v1::Status {
            device: v1::DeviceInfo {
                name: "kb".into(),
                battery: 80
            },
            events: vec![
                v1::UsbEvent::Connected,
                v1::UsbEvent::Unknown,
                v1::UsbEvent::Disconnected,
            ],
            uptime: 0x0102,
        }
    );
}

#[test]
fn undecodable_encodings() {
    // An enumerator past the known ones needs an unknown variant to land in.
    assert_eq!(
        ChargeState::decode_bytes(&[2, 0]),
        Err(Error::InvalidDiscriminant)
    );
    assert_eq!(
        PixelFormat::decode_bytes(&[9]),
        Err(Error::InvalidDiscriminant)
    );
    // Only fields with a missing value can be left out.
    assert_eq!(Process::decode_bytes(&[0]), Err(Error::UnexpectedEof));
    // A struct may end between fields, not inside one, even if the bytes after it would complete
    // the field.
    assert_eq!(
        v1::DeviceInfo::decode_bytes(&[2, 5, b'k', b'b', b'x', b'y', b'z', 80]),
        Err(Error::UnexpectedEof)
    );
    assert_eq!(
        Shape::decode_bytes(&IMAGE_SHAPE[..5]),
        Err(Error::UnexpectedEof)
    );
}
