use std::{collections::VecDeque, time::Instant};

use bytes::Bytes;
use ql_codec::{Decode, Reader};
use ql_common::StreamId;
use ql_wire::{self as wire, QlCrypto, SessionCloseCode};

use crate::{
    handshake,
    session::{self, SessionEvent},
    state::LinkState,
    Event, NoPeerError, NoSessionError, OpenStreamError, OutboundWrite, QlFsm, ReceiveError,
    ReceiveStage, StreamError, StreamMeta, WriteId,
};

pub struct EventSink<'a> {
    events: &'a mut VecDeque<Event>,
    terminal_event: Option<SessionEvent>,
}

impl<'a> EventSink<'a> {
    fn new(events: &'a mut VecDeque<Event>) -> Self {
        Self {
            events,
            terminal_event: None,
        }
    }
}

impl session::EventSink for EventSink<'_> {
    fn emit(&mut self, event: SessionEvent) {
        match event {
            SessionEvent::Opened(stream_id) => {
                self.events.push_back(Event::Opened(stream_id));
            }
            event @ (SessionEvent::SessionClosed { .. } | SessionEvent::Unpaired { .. }) => {
                self.terminal_event = Some(event);
            }
        }
    }
}

pub fn handle_bind_peer<M: StreamMeta>(fsm: &mut QlFsm<M>, peer: ql_wire::PeerBundle) {
    fsm.state.handshake = None;
    fsm.state.link = LinkState::Idle;
    fsm.state.peer = Some(peer);
}

pub fn unpair<M: StreamMeta>(fsm: &mut QlFsm<M>, crypto: &impl QlCrypto) {
    fsm.state.handshake = None;
    fsm.state.armed_pairing_token = None;

    if let Some(conn) = fsm.state.link.connected_mut() {
        let mut emit = EventSink::new(&mut fsm.events);
        conn.session.unpair(&mut emit);
        let event = emit.terminal_event;
        finish_termination(fsm, event, crypto);
    } else {
        fsm.state.link = LinkState::Idle;
        if fsm.state.peer.take().is_some() {
            fsm.events.push_back(Event::Unpaired { write: None });
            emit_peer_status(fsm, crate::PeerStatus::Unpaired);
        }
    }
}

pub fn handle_disarm_pairing<M: StreamMeta>(fsm: &mut QlFsm<M>) {
    fsm.state.armed_pairing_token = None;
    handshake::handle_disarm_pairing(fsm);
}

pub fn handle_connect_xx<M: StreamMeta>(
    fsm: &mut QlFsm<M>,
    invite: crate::PairingInvite,
    crypto: &impl QlCrypto,
) {
    handshake::handle_connect_xx(fsm, invite, crypto);
}

pub fn handle_connect_ik<M: StreamMeta>(
    fsm: &mut QlFsm<M>,
    crypto: &impl QlCrypto,
) -> Result<(), NoPeerError> {
    handshake::handle_connect_ik(fsm, crypto)
}

pub fn handle_connect_kk<M: StreamMeta>(
    fsm: &mut QlFsm<M>,
    crypto: &impl QlCrypto,
) -> Result<(), NoPeerError> {
    handshake::handle_connect_kk(fsm, crypto)
}

pub fn receive<M: StreamMeta, B>(
    fsm: &mut QlFsm<M>,
    mut bytes: B,
    crypto: &impl QlCrypto,
) -> Result<bool, ReceiveError>
where
    B: AsRef<[u8]> + AsMut<[u8]> + Send + 'static,
{
    let buffer = bytes.as_mut();
    let mut reader = Reader::new(buffer);
    let header = wire::RecordHeader::decode(&mut reader)
        .map_err(|error| ReceiveError::wire(ReceiveStage::RecordHeader, error))?;

    if header.version != wire::QL_WIRE_VERSION {
        return Err(ReceiveError::InvalidRecordVersion);
    }
    if header.route.recipient != fsm.identity.qid {
        return Err(ReceiveError::InvalidQid);
    }

    match header.record_type {
        wire::RecordType::Handshake => {
            let record = wire::QlHandshakeRecord::decode(&mut reader)
                .map_err(|error| ReceiveError::wire(ReceiveStage::HandshakeRecord, error))?;
            handshake::handle_handshake_record(fsm, crypto, header.route, &record)
        }
        wire::RecordType::Session => {
            let event = {
                let QlFsm { state, events, .. } = fsm;
                let conn = state.link.connected_mut_or_err()?;
                if header.route.sender != conn.transport.remote_qid {
                    return Err(ReceiveError::InvalidQid);
                }
                let (decrypted, seq) = {
                    // This decodes a borrowed `QlSessionRecord<&[u8]>`, without copying.
                    // `Vec<u8>` is named because `Decode` is implemented on the owned form.
                    let record = wire::QlSessionRecord::<Vec<u8>>::decode_ref(&mut reader)
                        .map_err(|error| ReceiveError::wire(ReceiveStage::SessionRecord, error))?;
                    if conn.session.is_replay(record.header.seq) {
                        return Ok(false);
                    }
                    let session_header = record.header;
                    let auth = record.payload.auth;
                    let ciphertext_address = record.payload.ciphertext.as_ptr() as usize;
                    let ciphertext_len = record.payload.ciphertext.len();

                    // XXX: The reader only lends shared slices, so the ciphertext is located in
                    // `buffer` by its address, to decrypt it in place.
                    let ciphertext_start = ciphertext_address - buffer.as_ptr() as usize;
                    let ciphertext_range = ciphertext_start..ciphertext_start + ciphertext_len;
                    wire::decrypt_record(
                        crypto,
                        &header,
                        &session_header,
                        wire::EncryptedMessage {
                            auth,
                            ciphertext: &mut buffer[ciphertext_range.clone()],
                        },
                        &conn.transport.rx_key,
                    )
                    .map_err(|error| ReceiveError::wire(ReceiveStage::SessionPayload, error))?;
                    (ciphertext_range, session_header.seq)
                };

                let bytes = Bytes::from_owner(bytes);
                let plaintext = bytes.slice(decrypted);

                let mut emit = EventSink::new(events);
                conn.session.receive(state.now, seq, &plaintext, &mut emit);
                emit.terminal_event
            };

            finish_termination(fsm, event, crypto);
            Ok(true)
        }
    }
}

pub fn on_timer<M: StreamMeta>(fsm: &mut QlFsm<M>, crypto: &impl QlCrypto) {
    handshake::handle_timer(fsm);

    let event = {
        let QlFsm { state, events, .. } = fsm;
        let Some(conn) = state.link.connected_mut() else {
            return;
        };

        let mut emit = EventSink::new(events);
        conn.session.on_timer(state.now, &mut emit);
        emit.terminal_event
    };
    finish_termination(fsm, event, crypto);
}

pub fn next_deadline<M: StreamMeta>(fsm: &QlFsm<M>) -> Option<Instant> {
    [
        handshake::next_handshake_deadline(fsm),
        fsm.state
            .link
            .connected()
            .and_then(|state| state.session.next_deadline()),
    ]
    .into_iter()
    .flatten()
    .min()
}

pub fn take_next_write<M: StreamMeta>(
    fsm: &mut QlFsm<M>,
    crypto: &impl QlCrypto,
) -> Option<OutboundWrite> {
    if let Some((route, record)) = fsm.state.handshake.take() {
        let record = wire::encode_record_vec(
            wire::RecordHeader::new(route, ql_wire::RecordType::Handshake),
            &record,
        );
        return Some(OutboundWrite {
            record,
            write_id: None,
        });
    }

    let QlFsm { state, .. } = fsm;
    let conn = state.link.connected_mut()?;
    let route = wire::RouteHeader {
        sender: fsm.identity.qid,
        recipient: conn.transport.remote_qid,
    };

    let (write_id, builder) = conn.session.take_next_write(state.now)?;
    let record = builder.encrypt(crypto, route, &conn.transport.tx_key);
    Some(OutboundWrite {
        record,
        write_id: write_id.map(WriteId),
    })
}

pub fn complete_write<M: StreamMeta>(fsm: &mut QlFsm<M>, write_id: WriteId, success: bool) {
    let QlFsm { state, .. } = fsm;
    if let Some(conn) = state.link.connected_mut() {
        conn.session.complete_write(state.now, write_id.0, success);
    }
}

pub fn close_session<M: StreamMeta>(
    fsm: &mut QlFsm<M>,
    code: SessionCloseCode,
    crypto: &impl QlCrypto,
) {
    let event = {
        let QlFsm { state, events, .. } = fsm;
        let Some(conn) = state.link.connected_mut() else {
            return;
        };
        let mut emit = EventSink::new(events);
        conn.session.close(code, &mut emit);
        emit.terminal_event
    };
    finish_termination(fsm, event, crypto);
}

pub fn open_stream<M: StreamMeta>(
    fsm: &mut QlFsm<M>,
    header: Box<[u8]>,
    options: crate::StreamOptions,
) -> Result<crate::StreamOps<'_, M>, OpenStreamError> {
    let QlFsm { state, .. } = fsm;
    let conn = state.link.connected_mut_or_err()?;
    conn.session.open_stream(header, options)
}

pub fn stream<M: StreamMeta>(
    fsm: &mut QlFsm<M>,
    stream_id: StreamId,
) -> Result<crate::StreamOps<'_, M>, StreamError> {
    let QlFsm { state, .. } = fsm;
    let conn = state.link.connected_mut_or_err()?;
    conn.session.stream(stream_id)
}

pub fn queue_ping<M: StreamMeta>(fsm: &mut QlFsm<M>) -> Result<(), NoSessionError> {
    let conn = fsm.state.link.connected_mut_or_err()?;
    conn.session.queue_ping();
    Ok(())
}

pub fn poll_event<M: StreamMeta>(fsm: &mut QlFsm<M>) -> Option<Event> {
    fsm.events.pop_front()
}

pub fn emit_peer_status<M: StreamMeta>(fsm: &mut QlFsm<M>, status: crate::PeerStatus) {
    fsm.events.push_back(Event::PeerStatusChanged(status));
}

fn finish_termination<M: StreamMeta>(
    fsm: &mut QlFsm<M>,
    event: Option<SessionEvent>,
    crypto: &impl QlCrypto,
) {
    let Some(event) = event else {
        return;
    };
    let LinkState::Connected(state) = fsm.state.link.take() else {
        unreachable!("terminal session event requires a connected session");
    };
    let route = wire::RouteHeader {
        sender: fsm.identity.qid,
        recipient: state.transport.remote_qid,
    };
    let encrypt = |write: wire::SessionRecordBuilder| {
        Box::new(OutboundWrite {
            record: write.encrypt(crypto, route, &state.transport.tx_key),
            write_id: None,
        })
    };

    match event {
        SessionEvent::SessionClosed { close, write } => {
            fsm.events.push_back(Event::SessionClosed {
                close,
                write: encrypt(write),
            });
            emit_peer_status(fsm, crate::PeerStatus::Disconnected);
        }
        SessionEvent::Unpaired { write } => {
            fsm.state.handshake = None;
            fsm.state.armed_pairing_token = None;
            fsm.state.peer = None;
            fsm.events.push_back(Event::Unpaired {
                write: Some(encrypt(write)),
            });
            emit_peer_status(fsm, crate::PeerStatus::Unpaired);
        }
        SessionEvent::Opened(_) => unreachable!("opened events are forwarded immediately"),
    }
}
