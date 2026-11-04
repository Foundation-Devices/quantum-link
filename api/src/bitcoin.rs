use ql_rpc::{Request, Subscription};

use crate::Empty;

// COMMON TYPES

rpc! {
    pub struct ActiveSeedFingerprint {
        pub fingerprint: String,
        pub has_passphrase: bool,
    }
}

// APP ROUTES

ql_keyos::app_routes! {
    crate::app_id::BITCOIN => {
        RequestSignPsbt: Request = 1,
        SubscribeAccountUpdated: Subscription = 2,
        RequestPassportActiveSeedFingerprint: Request = 3,
        SubscribePassportActiveSeedFingerprint: Subscription = 4,
    }
}

rpc! {
    pub struct SignPsbtParams {
        pub account_id: String,
        pub psbt: Vec<u8>,
    }
}

rpc! {
    #[repr(u32)]
    pub enum SignPsbtResponse {
        Signed { psbt: Vec<u8> } = 1,
        Rejected = 2,
        Error { error: String } = 3,
    }
}

impl Request for RequestSignPsbt {
    type Request = SignPsbtParams;
    type Response = SignPsbtResponse;
}

rpc! {
    pub struct AccountUpdatedEvent {
        pub account_id: String,
        pub update: Vec<u8>,
    }
}

impl Subscription for SubscribeAccountUpdated {
    type Request = Empty;
    type Event = AccountUpdatedEvent;
}

impl Subscription for SubscribePassportActiveSeedFingerprint {
    type Request = Empty;
    type Event = ActiveSeedFingerprint;
}

impl Request for RequestPassportActiveSeedFingerprint {
    type Request = Empty;
    type Response = ActiveSeedFingerprint;
}

// SERVICE ROUTES

ql_keyos::service_routes! {
    crate::service_id::BITCOIN => {
        RequestBroadcastTransaction: Request = 1,
    }
}

rpc! {
    pub struct BroadcastTransactionParams {
        pub account_id: String,
        pub psbt: Vec<u8>,
    }
}

rpc! {
    #[repr(u32)]
    pub enum BroadcastTransactionResponse {
        Broadcast { txid: String } = 1,
        Error { error: String } = 2,
    }
}

impl Request for RequestBroadcastTransaction {
    type Request = BroadcastTransactionParams;
    type Response = BroadcastTransactionResponse;
}
