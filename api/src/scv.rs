use ql_rpc::Duplex;

// APP ROUTES

// SERVICE ROUTES

ql_keyos::service_routes! {
    crate::service_id::SCV => {
        RunSecurityCheck: Duplex = 1,
    }
}

rpc! {
    pub struct ChallengeRequest {
        pub data: Vec<u8>,
    }
}

rpc! {
    #[repr(u32)]
    pub enum ChallengeResponseResult {
        Success { data: Vec<u8> } = 1,
        Error { error: String } = 2,
    }
}

rpc! {
    #[repr(u32)]
    pub enum VerificationResult {
        Success = 1,
        Error { error: String } = 2,
        Failure = 3,
    }
}

rpc! {
    #[repr(u32)]
    pub enum RunSecurityCheckResponderEvent {
        ChallengeRequest(ChallengeRequest) = 1,
        VerificationResult(VerificationResult) = 2,
    }
}

rpc! {
    #[repr(u32)]
    pub enum RunSecurityCheckInitiatorEvent {
        Start = 1,
        ChallengeResponse(ChallengeResponseResult) = 2,
    }
}

impl Duplex for RunSecurityCheck {
    type InitiatorEvent = RunSecurityCheckInitiatorEvent;
    type ResponderEvent = RunSecurityCheckResponderEvent;
}
