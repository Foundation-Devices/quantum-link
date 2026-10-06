use ql_common::{ResetCode, StreamId};
use ql_wire::StreamReset;

use super::state::{InboundState, OutboundState, StreamIoState, StreamState};
use crate::{CommitReadError, ResetOrigin, StreamMeta, StreamResetEvent, StreamResetTarget};

/// mutable access to one stream
/// terminal streams are reaped when the session is next polled
pub struct StreamOps<'a, M: StreamMeta> {
    stream_id: StreamId,
    stream: &'a mut StreamState<M>,
}

impl<'a, M: StreamMeta> StreamOps<'a, M> {
    pub(super) fn new(stream_id: StreamId, stream: &'a mut StreamState<M>) -> Self {
        Self { stream_id, stream }
    }

    pub fn metadata(&self) -> &M {
        &self.stream.metadata
    }

    pub fn metadata_mut(&mut self) -> &mut M {
        &mut self.stream.metadata
    }

    pub fn io(&mut self) -> StreamIo<'_> {
        StreamIo::new(self.stream_id, &mut self.stream.io)
    }

    /// returns the metadata and stream I/O together
    pub fn split_mut(&mut self) -> (&mut M, StreamIo<'_>) {
        (
            &mut self.stream.metadata,
            StreamIo::new(self.stream_id, &mut self.stream.io),
        )
    }

    #[inline]
    pub fn poll_readable(&mut self) {
        let (metadata, io) = self.split_mut();
        metadata.on_readable(io);
    }

    #[inline]
    pub fn poll_writable(&mut self) {
        let (metadata, io) = self.split_mut();
        metadata.on_writable(io);
    }

    /// resets the stream and notifies its metadata
    pub fn reset(&mut self, target: StreamResetTarget, code: ResetCode) {
        let stream_id = self.stream_id;
        let StreamState { metadata, io } = &mut *self.stream;
        let wire_target = match target {
            StreamResetTarget::Reader => io.role.inbound_target(),
            StreamResetTarget::Writer => io.role.outbound_target(),
            StreamResetTarget::Both => ql_wire::ResetTarget::Both,
        };
        let reset = StreamReset {
            stream_id,
            target: wire_target,
            code,
        };
        if target.reader() {
            io.inbound_state = InboundState::Reset(reset.clone());
            io.reset_recv();
        }
        if target.writer() {
            io.outbound_state = OutboundState::Reset(reset.clone());
            io.tx.clear();
        }
        io.pending_reset = Some(reset);
        metadata.on_reset(StreamResetEvent {
            stream_id,
            code,
            target,
            origin: ResetOrigin::Local,
        });
    }
}

/// I/O operations for one stream
pub struct StreamIo<'a> {
    stream_id: StreamId,
    state: &'a mut StreamIoState,
}

impl<'a> StreamIo<'a> {
    pub(super) fn new(stream_id: StreamId, state: &'a mut StreamIoState) -> Self {
        Self { stream_id, state }
    }

    pub fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    pub fn header(&self) -> &[u8] {
        self.state.header.as_ref().unwrap()
    }

    pub fn reader(&mut self) -> ReaderState<'_> {
        if let InboundState::Reset(reset) = &self.state.inbound_state {
            return ReaderState::Reset(reset.code);
        }
        let readable = self.state.readable_bytes() > 0;
        let finished = matches!(self.state.inbound_state, InboundState::Finished);
        match (readable, finished) {
            (true, true) => ReaderState::Final(StreamReader { state: self.state }),
            (true, false) => ReaderState::Readable(StreamReader { state: self.state }),
            (false, true) => ReaderState::Finished,
            (false, false) => ReaderState::Open,
        }
    }

    pub fn writer(&mut self) -> WriterState<'_> {
        if let OutboundState::Reset(reset) = &self.state.outbound_state {
            return WriterState::Reset(reset.code);
        }
        if matches!(
            self.state.outbound_state,
            OutboundState::FinQueued | OutboundState::Finished
        ) {
            return WriterState::Finished;
        }
        WriterState::Open(StreamWriter { state: self.state })
    }
}

pub enum ReaderState<'a> {
    Open,
    Readable(StreamReader<'a>),
    Final(StreamReader<'a>),
    Finished,
    Reset(ResetCode),
}

impl<'a> ReaderState<'a> {
    pub fn active(self) -> Option<StreamReader<'a>> {
        match self {
            Self::Readable(reader) | Self::Final(reader) => Some(reader),
            Self::Open | Self::Finished | Self::Reset(_) => None,
        }
    }
}

pub enum WriterState<'a> {
    Open(StreamWriter<'a>),
    Finished,
    Reset(ResetCode),
}

impl<'a> WriterState<'a> {
    pub fn active(self) -> Option<StreamWriter<'a>> {
        match self {
            Self::Open(writer) => Some(writer),
            Self::Finished | Self::Reset(_) => None,
        }
    }
}

pub struct StreamReader<'a> {
    state: &'a mut StreamIoState,
}

impl StreamReader<'_> {
    /// returns the readable stream bytes as owned `Bytes` views without consuming them
    pub fn read(&self) -> impl Iterator<Item = bytes::Bytes> + '_ {
        self.state.rx.bytes()
    }

    /// returns how many bytes can be read from the stream
    pub fn readable_bytes(&self) -> usize {
        self.state.readable_bytes()
    }

    /// marks previously read bytes as consumed
    pub fn commit_read(&mut self, len: usize) -> Result<(), CommitReadError> {
        if len > self.state.readable_bytes() {
            return Err(CommitReadError);
        }
        self.state.rx.consume(len);
        if matches!(self.state.inbound_state, InboundState::Open)
            && self.state.recv_limit() > self.state.advertised_max_offset
        {
            self.state.pending_window = true;
        }
        Ok(())
    }
}

pub struct StreamWriter<'a> {
    state: &'a mut StreamIoState,
}

impl StreamWriter<'_> {
    /// returns how many bytes can still be buffered for local writes
    pub fn capacity(&self) -> usize {
        self.state.send_capacity()
    }

    /// appends as many bytes as possible and returns the accepted count
    pub fn write(&mut self, bytes: &mut bytes::Bytes) -> usize {
        let accepted = bytes.len().min(self.capacity());
        if accepted > 0 {
            self.state.tx.append(bytes.split_to(accepted));
        }
        accepted
    }

    /// marks the local write side as finished
    pub fn finish(self) {
        self.state.tx.queue_fin();
        self.state.outbound_state = OutboundState::FinQueued;
    }
}
