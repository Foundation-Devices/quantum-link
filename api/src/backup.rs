use ql_rpc::{Download, Request, Subscription, Upload};

use crate::Empty;

// COMMON TYPES

rpc! {
    pub struct Shard(pub Vec<u8>);
}

rpc! {
    pub struct SeedFingerprint(pub [u8; 32]);
}

// APP ROUTES

app_routes! {
    crate::app_id::BACKUP => {
        SubscribePassportMagicBackupEnabled: Subscription = 1,
        SubscribeMagicBackupRestoreCompleted: Subscription = 2,
    }
}

rpc! {
    pub struct PassportMagicBackupEnabled {
        pub enabled: bool,
        pub seed_fingerprint: SeedFingerprint,
    }
}

impl Subscription for SubscribePassportMagicBackupEnabled {
    type Request = Empty;
    type Event = PassportMagicBackupEnabled;
}

rpc! {
    #[repr(u32)]
    pub enum MagicBackupRestoreResult {
        Success = 1,
        Error { error: String } = 2,
    }
}

impl Subscription for SubscribeMagicBackupRestoreCompleted {
    type Request = Empty;
    type Event = MagicBackupRestoreResult;
}

// SERVICE ROUTES

service_routes! {
    crate::service_id::BACKUP => {
        RequestBackupShard: Request = 1,
        RequestRestoreShard: Request = 2,
        RequestPassportMagicBackupStatus: Request = 3,
        UploadMagicBackup: Upload = 4,
        DownloadMagicBackup: Download = 5,
    }
}

rpc! {
    pub struct BackupShardParams {
        pub shard: Shard,
    }
}

rpc! {
    #[repr(u32)]
    pub enum BackupShardResponse {
        Success = 1,
        Error { error: String } = 2,
    }
}

impl Request for RequestBackupShard {
    type Request = BackupShardParams;
    type Response = BackupShardResponse;
}

rpc! {
    pub struct RestoreShardParams {
        pub seed_fingerprint: SeedFingerprint,
        pub timestamp: Option<u32>,
    }
}

rpc! {
    #[repr(u32)]
    pub enum RestoreShardResponse {
        Success { shard: Shard } = 1,
        Error { error: String } = 2,
        NotFound = 3,
    }
}

impl Request for RequestRestoreShard {
    type Request = RestoreShardParams;
    type Response = RestoreShardResponse;
}

rpc! {
    pub struct PassportMagicBackupStatusParams {
        pub seed_fingerprint: SeedFingerprint,
        pub timestamp: Option<u32>,
    }
}

rpc! {
    pub struct PassportMagicBackupStatusResponse {
        pub shard_backup_found: bool,
    }
}

impl Request for RequestPassportMagicBackupStatus {
    type Request = PassportMagicBackupStatusParams;
    type Response = PassportMagicBackupStatusResponse;
}

rpc! {
    pub struct UploadMagicBackupParams {
        pub seed_fingerprint: SeedFingerprint,
        pub total_size: Option<u64>,
        pub hash: [u8; 32],
    }
}

rpc! {
    #[repr(u32)]
    pub enum UploadMagicBackupResponse {
        Success = 1,
        Error { error: String } = 2,
    }
}

rpc! {
    pub struct UploadMagicBackupPartHeader {}
}

impl Upload for UploadMagicBackup {
    type Request = UploadMagicBackupParams;
    type PartHeader = UploadMagicBackupPartHeader;
    type Response = UploadMagicBackupResponse;
}

rpc! {
    pub struct DownloadMagicBackupParams {
        pub seed_fingerprint: SeedFingerprint,
    }
}

rpc! {
    pub struct BackupMetadata {
        pub total_size: Option<u64>,
    }
}

rpc! {
    #[repr(u32)]
    pub enum DownloadMagicBackupHeader {
        Found(BackupMetadata) = 1,
        NotFound = 2,
        Error { error: String } = 3,
    }
}

impl Download for DownloadMagicBackup {
    type Request = DownloadMagicBackupParams;
    type ResponseHeader = DownloadMagicBackupHeader;
    type PartHeader = Empty;
}
