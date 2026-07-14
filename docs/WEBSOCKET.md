# TITAN WebSockets

TITAN implements the RFC 6455 handshake and frame codec. `accept_key` validates that the client nonce decodes to exactly 16 bytes and computes the standard SHA-1/GUID response. `upgrade_response` emits a complete 101 response with an optional validated subprotocol.

`std::ws::encode(opcode, payload, masked)` builds minimally encoded frames. Client masking keys come from the operating system CSPRNG through `getrandom`; server frames remain unmasked. `std::ws::parse(bytes, require_mask, max_payload)` incrementally returns `Option`, unmasks payloads, reports consumed bytes, and enforces payload limits.

The codec rejects RSV bits without negotiated extensions, reserved opcodes, non-minimal lengths, invalid 64-bit lengths, masking-policy violations, fragmented/oversized control frames, one-byte close payloads, and invalid UTF-8 text frames. Supported opcodes are continuation, text, binary, close, ping, and pong.

`validate_upgrade(request, selected_protocol)` requires GET HTTP/1.1, Upgrade/Connection tokens, version 13, a valid key, and verifies that any selected subprotocol was offered before returning the 101 bytes. `validate_accept(raw_response, client_key)` requires a unique set of critical response headers and constant expected accept value before a client attaches its transport.

`std::ws::decoder(maximum)`, `decoder_push(decoder, bytes)`, and `decoder_next(decoder, require_mask)` expose a stateful VM-managed decoder. It reassembles fragmented text/binary messages across arbitrary network reads, allows ping/pong/close control frames between fragments, validates final UTF-8, tracks accumulated payload limits, validates close code/reason, and rejects unexpected continuations or interleaved data messages.

Messages are returned as maps with type-specific `text`, `data`, `code`, and `reason` fields. The decoder handle is share-safe but not JSON serializable.

## High-level connection

`attach_tcp(stream, server_side, maximum)` and `attach_tls(...)` transfer ownership of an existing transport into a WebSocket handle. `send_text`, `send_binary`, `receive`, and `close` provide message-level I/O. Client frames use secure random masking; server frames do not. Receive automatically answers ping with pong, preserves fragmented-message state, and mirrors a peer close before returning it. Close is idempotent, validates code/reason, removes the handle, and releases the underlying transport.

Transport locks are separate from decoder state so a blocking receive does not hold the global registry lock. The same connection object works over plain TCP or validated rustls streams.

## High-level client

`std::ws::connect(url, protocol, maximum)` accepts `ws://` and `wss://` URLs (including bracketed IPv6), applies default ports, generates a 16-byte CSPRNG nonce, performs the HTTP Upgrade, validates status/critical headers/accept key/subprotocol, preserves any frame bytes read with the handshake, and returns a connected WebSocket. Userinfo, fragments, control characters, invalid ports and unrequested subprotocols are rejected before attach.
