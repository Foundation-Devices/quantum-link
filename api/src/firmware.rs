use ql_rpc::{Download, Request, Subscription};

use crate::Empty;

// COMMON TYPES

rpc! {
    pub struct FirmwareUpdateAvailable {
        pub version: String,
        pub changelog: String,
        pub timestamp: u32,
        pub total_size: u32,
        pub patch_count: u8,
    }
}

// APP ROUTES

app_routes! {
    crate::app_id::UPDATE => {
        SubscribeFirmwareInstallStatus: Subscription = 1,
    }
}

rpc! {
    #[repr(u32)]
    pub enum InstallErrorStage {
        Download = 1,
        Verify = 2,
        Install = 3,
        #[codec(unknown)]
        Unknown = 0,
    }
}

rpc! {
    #[repr(u32)]
    pub enum FirmwareInstallEvent {
        Installing = 1,
        Rebooting = 2,
        Success {
            installed_version: String,
        } = 3,
        Error {
            error: String,
            stage: InstallErrorStage,
        } = 4,
        #[codec(unknown)]
        Unknown = 0,
    }
}

impl Subscription for SubscribeFirmwareInstallStatus {
    type Request = Empty;
    type Event = FirmwareInstallEvent;
}

// SERVICE ROUTES

service_routes! {
    crate::service_id::FIRMWARE => {
        RequestCheckFirmwareUpdate: Request = 1,
        DownloadFirmware: Download = 2,
    }
}

rpc! {
    pub struct FirmwareUpdateCheckParams {
        pub current_version: String,
    }
}

rpc! {
    #[repr(u32)]
    pub enum FirmwareUpdateCheckResponse {
        Available(FirmwareUpdateAvailable) = 1,
        NotAvailable = 2,
    }
}

impl Request for RequestCheckFirmwareUpdate {
    type Request = FirmwareUpdateCheckParams;
    type Response = FirmwareUpdateCheckResponse;
}

rpc! {
    pub struct DownloadFirmwareParams {
        pub current_version: String,
    }
}

rpc! {
    #[repr(u32)]
    pub enum DownloadFirmwareHeader {
        Available(FirmwareUpdateAvailable) = 1,
        NotAvailable = 2,
        Error { error: String } = 3,
    }
}

rpc! {
    pub struct DownloadFirmwarePartHeader {
        pub patch_name: String,
        pub size_bytes: u64,
    }
}

impl Download for DownloadFirmware {
    type Request = DownloadFirmwareParams;
    type ResponseHeader = DownloadFirmwareHeader;
    type PartHeader = DownloadFirmwarePartHeader;
}
