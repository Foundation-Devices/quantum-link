# Quantum Link

Quantum Link is a networking protocol for end-to-end encrypted communication. Authenticated sessions carry reliable, multiplexed byte streams.

- **Transport agnostic**
  - Any transport that delivers complete records can carry Quantum Link
  - `ql-btp` splits records into chunks for links with a limited MTU
- **Forward secret sessions**
  - Noise-style handshakes using ML-KEM-1024 establish fresh session keys
  - Communication is encrypted with AES-256-GCM
  - `IK` and `KK` connect known peers
  - `XX` pairs new peers using an out-of-band token
- **Routable records**
  - Visible sender and recipient identifiers allow routing without decrypting payloads
  - The routing metadata is authenticated
- **Reliable streams**
  - Each session multiplexes bidirectional byte streams with per-stream flow control
  - Applications can layer their own encoding and RPC on top

## Crates

- **ql-btp**: QuantumLink BTP framing for splitting records into MTU-sized chunks
- **ql-codec**: Binary codec primitives
- **ql-common**: Shared protocol types
- **ql-wire**: QuantumLink wire-format definitions
- **ql-fsm**: QuantumLink Sans-IO protocol finite state machine
- **ql-runtime**: QuantumLink async runtime
- **ql-rpc**: RPC modality layer over QuantumLink streams
- **ql-router**: Router protocol and client
- **ql-api**: Application message definitions
- **ql-keyos**: KeyOS addressing primitives

## License

This project is licensed under either the [MIT license](LICENSE-MIT) or the [Apache License 2.0](LICENSE-APACHE), at your option.
