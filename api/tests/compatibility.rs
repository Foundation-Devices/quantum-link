use bytes::{Buf, BytesMut};
use ql_api::*;
use ql_codec::{Codec, Decode, Encode};
use ql_rpc::RpcCodec;

#[test]
fn wire_format_is_stable() {
    let mut bytes = BytesMut::new();
    EchoParams {
        message: "hi".into(),
    }
    .encode_value(&mut bytes);
    assert_eq!(&bytes[..], &[3, 2, b'h', b'i']);
    assert_eq!(PassportModel::Prime.encode_vec(), [3, 0]);
    assert_eq!(VerificationResult::Success.encode_vec(), [1, 0]);
    assert_eq!(Empty {}.encode_vec(), [0]);

    let rate = ExchangeRate {
        currency_code: "USD".into(),
        rate: 1.5,
        timestamp: 42,
    };
    assert_eq!(
        rate.encode_vec(),
        [16, 3, b'U', b'S', b'D', 0, 0, 192, 63, 42, 0, 0, 0, 0, 0, 0, 0]
    );
    let mut bytes = BytesMut::new();
    rate.encode_value(&mut bytes);
    assert_eq!(ExchangeRate::decode_value(&mut bytes).unwrap(), rate);
}

#[test]
fn fields_can_be_appended_in_both_directions() {
    #[derive(Codec)]
    struct FutureEchoParams {
        message: String,
        locale: Option<String>,
    }

    let old = EchoParams {
        message: "hi".into(),
    };
    let bytes = old.encode_vec();
    let new = FutureEchoParams::decode_bytes(bytes.as_slice()).unwrap();
    assert_eq!(new.message, old.message);
    assert_eq!(new.locale, None);

    let bytes = FutureEchoParams {
        message: old.message.clone(),
        locale: Some("en".into()),
    }
    .encode_vec();
    assert_eq!(
        EchoParams::decode_value(&mut bytes.as_slice()).unwrap(),
        old
    );
}

#[test]
fn empty_headers_and_nested_permissions_can_grow() {
    #[derive(Codec)]
    struct FutureHeader {
        field: Option<u32>,
    }

    let bytes = Empty {}.encode_vec();
    assert_eq!(
        FutureHeader::decode_bytes(bytes.as_slice()).unwrap().field,
        None
    );
    let bytes = FutureHeader { field: Some(7) }.encode_vec();
    assert_eq!(
        Empty::decode_value(&mut bytes.as_slice()).unwrap(),
        Empty {}
    );
    assert_eq!(
        DownloadAppPartHeader::decode_value(&mut bytes.as_slice()).unwrap(),
        DownloadAppPartHeader {}
    );
    assert_eq!(
        UploadMagicBackupPartHeader::decode_value(&mut bytes.as_slice()).unwrap(),
        UploadMagicBackupPartHeader {}
    );
    assert_eq!(
        DownloadBenchmarkPartHeader::decode_value(&mut bytes.as_slice()).unwrap(),
        DownloadBenchmarkPartHeader {}
    );

    #[derive(Codec)]
    struct FuturePermissions {
        app_ids: Vec<ql_keyos::AppId>,
        capability: Option<String>,
    }

    #[derive(Codec)]
    struct FutureParams(FuturePermissions);

    let old = PeerPermissionsParams(ql_keyos::PeerPermissions {
        app_ids: vec![app_id::SYSTEM],
    });
    let bytes = old.encode_vec();
    let new = FutureParams::decode_bytes(bytes.as_slice()).unwrap();
    assert_eq!(new.0.app_ids, old.0.app_ids);
    assert_eq!(new.0.capability, None);
    let bytes = FutureParams(FuturePermissions {
        app_ids: new.0.app_ids,
        capability: Some("future".into()),
    })
    .encode_vec();
    assert_eq!(
        PeerPermissionsParams::decode_value(&mut bytes.as_slice()).unwrap(),
        old
    );
}

#[test]
fn unknown_events_and_descriptive_values_skip_their_payloads() {
    let mut bytes = &[127, 3, 1, 2, 3][..];
    assert_eq!(
        AppActivityEvent::decode_value(&mut bytes).unwrap(),
        AppActivityEvent::Unknown
    );
    assert!(!bytes.has_remaining());

    let mut bytes = &[4, 6, 0, 127, 3, 1, 2, 3][..];
    assert_eq!(
        FirmwareInstallEvent::decode_value(&mut bytes).unwrap(),
        FirmwareInstallEvent::Error {
            error: String::new(),
            stage: InstallErrorStage::Unknown,
        }
    );
    assert!(!bytes.has_remaining());

    let mut bytes = &[1, 3, 1, 2, 3][..];
    assert_eq!(
        VerificationResult::decode_value(&mut bytes).unwrap(),
        VerificationResult::Success
    );
}

#[test]
fn unknown_operation_outcomes_and_protocol_commands_are_rejected() {
    let bytes = &[127, 3, 1, 2, 3][..];
    assert_eq!(
        SignPsbtResponse::decode_value(&mut &bytes[..]),
        Err(Error::InvalidDiscriminant)
    );
    assert_eq!(
        MagicBackupRestoreResult::decode_value(&mut &bytes[..]),
        Err(Error::InvalidDiscriminant)
    );
    assert_eq!(
        RunSecurityCheckInitiatorEvent::decode_value(&mut &bytes[..]),
        Err(Error::InvalidDiscriminant)
    );
    assert_eq!(
        RunSecurityCheckResponderEvent::decode_value(&mut &bytes[..]),
        Err(Error::InvalidDiscriminant)
    );
    assert_eq!(
        RunSecurityCheckResponderEvent::decode_value(&mut &[2, 5, 127, 3, 1, 2, 3][..]),
        Err(Error::InvalidDiscriminant)
    );
}

#[test]
fn fragmented_payloads_decode_and_invalid_frames_fail() {
    let value = EchoParams {
        message: "hi".into(),
    };
    let bytes = value.encode_vec();
    for split in 0..=bytes.len() {
        let mut input = bytes[..split].chain(&bytes[split..]);
        assert_eq!(EchoParams::decode_value(&mut input).unwrap(), value);
        assert!(!input.has_remaining());
    }

    for bytes in [
        &[4, 2, b'h', b'i'][..],
        &[3, 3, b'h', b'i'],
        &[3, 2, 255, 255],
        &[3, 2, b'h', b'i', 0],
    ] {
        for split in 0..=bytes.len() {
            let mut input = bytes[..split].chain(&bytes[split..]);
            assert!(EchoParams::decode_value(&mut input).is_err());
        }
    }
    assert_eq!(
        AppActivityEvent::decode_value(&mut &[127, 3, 1][..]),
        Err(Error::UnexpectedEof)
    );
}
