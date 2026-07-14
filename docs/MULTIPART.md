# TITAN Multipart Uploads

`std::http::parse_multipart(content_type, body, max_parts, max_part_bytes)` parses bounded `multipart/form-data` bodies and returns part maps containing `name`, optional `filename`, optional `content_type`, normalized `headers`, and binary `data`.

The parser validates boundary syntax/length, opening/final delimiters, CRLF framing, per-part headers (16 KiB/32 header hard limits), Content-Disposition, requested part count and per-part byte limits. Filenames containing path separators, CR/LF or traversal components are rejected rather than sanitized ambiguously.

The API keeps uploaded file data in memory only up to explicit limits. Applications should write accepted bytes to a server-generated destination name; client filenames must never be used as filesystem paths.
