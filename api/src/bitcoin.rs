use ql_rpc::Request;

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
