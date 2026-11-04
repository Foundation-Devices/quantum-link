use ql_rpc::Subscription;

use crate::Empty;

// APP ROUTES

app_routes! {
    crate::app_id::ONBOARDING => {
        SubscribeOnboardingStatus: Subscription = 1,
    }
}

rpc! {
    #[repr(u32)]
    pub enum OnboardingState {
        FirmwareUpdateScreen = 1,
        SecuringDevice = 2,
        DeviceSecured = 3,
        WalletCreationScreen = 4,
        CreatingWallet = 5,
        WalletCreated = 6,
        MagicBackupScreen = 7,
        CreatingMagicBackup = 8,
        MagicBackupCreated = 9,
        CreatingManualBackup = 10,
        CreatingKeycardBackup = 11,
        WritingDownSeedWords = 12,
        Completed = 13,
        #[codec(unknown)]
        Unknown = 0,
    }
}

impl Subscription for SubscribeOnboardingStatus {
    type Request = Empty;
    type Event = OnboardingState;
}

// SERVICE ROUTES
