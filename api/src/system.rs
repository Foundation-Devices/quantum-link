use ql_keyos::{AppId, PeerPermissions};
use ql_rpc::{Request, Subscription};

use crate::Empty;

// COMMON TYPES

rpc! {
    #[repr(u32)]
    pub enum PassportModel {
        Gen1 = 1,
        Gen2 = 2,
        Prime = 3,
        #[codec(unknown)]
        Unknown = 0,
    }
}

rpc! {
    pub struct PassportFirmwareVersion(pub String);
}

rpc! {
    pub struct PassportSerial(pub String);
}

rpc! {
    #[repr(u32)]
    pub enum PassportColor {
        Light = 1,
        Dark = 2,
        #[codec(unknown)]
        Unknown = 0,
    }
}

// APP ROUTES

app_routes! {
    crate::app_id::SYSTEM => {
        RequestPassportDetails: Request = 1,
        SubscribePassportDetails: Subscription = 2,
        RequestPassportFiatPreference: Request = 3,
        SubscribePassportFiatPreference: Subscription = 4,
        SubscribeAppActivity: Subscription = 5,
        RequestPeerPermissions: Request = 6,
        RequestInstallPeerBundles: Request = 7,
        RequestAttachRouter: Request = 8,
    }
}

rpc! {
    pub struct PassportDetailsResponse {
        pub device_name: String,
        pub model: PassportModel,
        pub firmware_version: PassportFirmwareVersion,
        pub serial: PassportSerial,
        pub color: PassportColor,
        pub onboarding_complete: bool,
    }
}

impl Request for RequestPassportDetails {
    type Request = Empty;
    type Response = PassportDetailsResponse;
}

impl Subscription for SubscribePassportDetails {
    type Request = Empty;
    type Event = PassportDetailsResponse;
}

rpc! {
    pub struct PassportFiatPreferenceResponse {
        pub currency_code: String,
    }
}

impl Request for RequestPassportFiatPreference {
    type Request = Empty;
    type Response = PassportFiatPreferenceResponse;
}

rpc! {
    pub struct PassportFiatPreferenceEvent {
        pub currency_code: String,
    }
}

impl Subscription for SubscribePassportFiatPreference {
    type Request = Empty;
    type Event = PassportFiatPreferenceEvent;
}

rpc! {
    #[repr(u32)]
    pub enum AppActivityEvent {
        Opened(AppId) = 1,
        Closed(AppId) = 2,
        #[codec(unknown)]
        Unknown = 0,
    }
}

impl Subscription for SubscribeAppActivity {
    type Request = Empty;
    type Event = AppActivityEvent;
}

rpc! {
    pub struct PeerPermissionsParams(pub PeerPermissions);
}

rpc! {
    #[repr(u32)]
    pub enum PeerPermissionsResponse {
        Updated = 1,
        Denied = 2,
    }
}

impl Request for RequestPeerPermissions {
    type Request = PeerPermissionsParams;
    type Response = PeerPermissionsResponse;
}

rpc! {
    pub struct InstallPeerBundlesParams {
        pub router: Vec<u8>,
        pub peers: Vec<Vec<u8>>,
    }
}

rpc! {
    #[repr(u32)]
    pub enum InstallPeerBundlesResponse {
        Installed = 1,
        Rejected = 2,
    }
}

impl Request for RequestInstallPeerBundles {
    type Request = InstallPeerBundlesParams;
    type Response = InstallPeerBundlesResponse;
}

rpc! {
    #[repr(u32)]
    pub enum AttachRouterResponse {
        Started = 1,
        Rejected = 2,
    }
}

impl Request for RequestAttachRouter {
    type Request = Empty;
    type Response = AttachRouterResponse;
}

// SERVICE ROUTES
