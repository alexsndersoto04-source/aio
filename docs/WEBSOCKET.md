# TITAN WebSockets

TITAN implements the RFC 6455 handshake and frame codec. `accept_key` validates that the client nonce decodes to exactly 16 bytes and computes the standard SHA-1/GUID response. `upgrade_response` emits a complete 101 response with an optional validated subprotocol.

`std::ws::encode(opcode, payload, masked)` builds minimally encoded frames. Client masking keys come from the operating system CSPRNG through `getrandom`; server frames remain unmasked. `std::ws::parse(bytes, require_mask, max_payload)` incrementally returns `Option`, unmasks payloads, reports consumed bytes, and enforces payload limits.

The codec rejects RSV bits without negotiated extensions, reserved opcodes, non-minimal lengths, invalid 64-bit lengths, masking-policy violations, fragmented/oversized control frames, one-byte close payloads, and invalid UTF-8 text frames. Supported opcodes are continuation, text, binary, close, ping, and pong.

`std::ws::decoder(maximum)`, `decoder_push(decoder, bytes)`, and `decoder_next(decoder, require_mask)` expose a stateful VM-managed decoder. It reassembles fragmented text/binary messages across arbitrary network reads, allows ping/pong/close control frames between fragments, validates final UTF-8, tracks accumulated payload limits, validates close code/reason, and rejects unexpected continuations or interleaved data messages.

Messages are returned as maps with type-specific `text`, `data`, `code`, and `reason` fields. The decoder handle is share-safe but not JSON serializable. These primitives work over both TCP and TLS stream reads/writes. Automatic ping/pong and the high-level close handshake are the next WebSocket connection layer.
