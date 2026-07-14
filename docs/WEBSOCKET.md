# TITAN WebSockets

TITAN implements the RFC 6455 handshake and frame codec. `accept_key` validates that the client nonce decodes to exactly 16 bytes and computes the standard SHA-1/GUID response. `upgrade_response` emits a complete 101 response with an optional validated subprotocol.

`std::ws::encode(opcode, payload, masked)` builds minimally encoded frames. Client masking keys come from the operating system CSPRNG through `getrandom`; server frames remain unmasked. `std::ws::parse(bytes, require_mask, max_payload)` incrementally returns `Option`, unmasks payloads, reports consumed bytes, and enforces payload limits.

The codec rejects RSV bits without negotiated extensions, reserved opcodes, non-minimal lengths, invalid 64-bit lengths, masking-policy violations, fragmented/oversized control frames, one-byte close payloads, and invalid UTF-8 text frames. Supported opcodes are continuation, text, binary, close, ping, and pong.

These primitives work over both TCP and TLS stream reads/writes. Stateful fragmented-message reassembly and the high-level connection API are the next WebSocket layer.
