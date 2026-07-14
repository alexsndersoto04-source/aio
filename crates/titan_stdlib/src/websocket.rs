//! RFC 6455 WebSocket handshake and frame codec.

use sha1::{Digest, Sha1};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)] pub enum WebSocketError {
    #[error("invalid Sec-WebSocket-Key")] InvalidKey,
    #[error("reserved WebSocket bits are set")] ReservedBits,
    #[error("unsupported WebSocket opcode {0}")] Opcode(u8),
    #[error("WebSocket masking policy violation")] Masking,
    #[error("WebSocket control frame is invalid")] ControlFrame,
    #[error("WebSocket payload exceeds configured limit")] TooLarge,
    #[error("WebSocket text payload is not UTF-8")] InvalidUtf8,
    #[error("WebSocket frame length is not minimally encoded")] NonMinimalLength,
    #[error("secure random masking key generation failed")] Entropy,
    #[error("unexpected WebSocket continuation frame")] UnexpectedContinuation,
    #[error("new data frame received during fragmented message")] InterleavedFragment,
    #[error("invalid WebSocket close code or reason")] InvalidClose,
}
#[derive(Debug, Clone, PartialEq, Eq)] pub struct Frame { pub fin: bool, pub opcode: u8, pub payload: Vec<u8>, pub consumed: usize }
#[derive(Debug, Clone, PartialEq, Eq)] pub enum Message { Text(String), Binary(Vec<u8>), Ping(Vec<u8>), Pong(Vec<u8>), Close { code: Option<u16>, reason: String } }
pub struct MessageDecoder { buffer: Vec<u8>, fragmented_opcode: Option<u8>, fragmented: Vec<u8>, maximum: usize }
impl MessageDecoder {
    pub fn new(maximum: usize) -> Self { Self { buffer: Vec::new(), fragmented_opcode: None, fragmented: Vec::new(), maximum } }
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), WebSocketError> { if self.buffer.len().saturating_add(bytes.len()) > self.maximum.saturating_add(14) { return Err(WebSocketError::TooLarge); } self.buffer.extend_from_slice(bytes); Ok(()) }
    pub fn next(&mut self, require_mask: Option<bool>) -> Result<Option<Message>, WebSocketError> {
        loop {
            let Some(frame) = parse_frame(&self.buffer, require_mask, self.maximum)? else { return Ok(None) };
            self.buffer.drain(..frame.consumed);
            match frame.opcode {
                0 => { let opcode = self.fragmented_opcode.ok_or(WebSocketError::UnexpectedContinuation)?; self.append_fragment(&frame.payload)?; if frame.fin { let payload=std::mem::take(&mut self.fragmented);self.fragmented_opcode=None;return decode_data(opcode,payload).map(Some); } }
                opcode @ 1..=2 => {
                    if self.fragmented_opcode.is_some() { return Err(WebSocketError::InterleavedFragment); }
                    if frame.fin { return decode_data(opcode, frame.payload).map(Some); }
                    self.fragmented_opcode = Some(opcode); self.append_fragment(&frame.payload)?;
                }
                8 => return decode_close(frame.payload).map(Some),
                9 => return Ok(Some(Message::Ping(frame.payload))),
                10 => return Ok(Some(Message::Pong(frame.payload))),
                _ => unreachable!(),
            }
        }
    }
    fn append_fragment(&mut self, payload:&[u8])->Result<(),WebSocketError>{if self.fragmented.len().saturating_add(payload.len())>self.maximum{return Err(WebSocketError::TooLarge)}self.fragmented.extend_from_slice(payload);Ok(())}
}

pub fn accept_key(client_key: &str) -> Result<String, WebSocketError> {
    let decoded = crate::encoding::base64_decode(client_key.trim()).map_err(|_| WebSocketError::InvalidKey)?;
    if decoded.len() != 16 { return Err(WebSocketError::InvalidKey); }
    let mut hash = Sha1::new(); hash.update(client_key.trim().as_bytes()); hash.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    Ok(crate::encoding::base64_encode(&hash.finalize()))
}
pub fn upgrade_response(client_key: &str, protocol: Option<&str>) -> Result<Vec<u8>, WebSocketError> {
    let accept = accept_key(client_key)?; let mut response = format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n");
    if let Some(protocol) = protocol { if !valid_token(protocol) { return Err(WebSocketError::InvalidKey); } response.push_str(&format!("Sec-WebSocket-Protocol: {protocol}\r\n")); }
    response.push_str("\r\n"); Ok(response.into_bytes())
}
pub fn encode_frame_with_policy(opcode: u8, payload: &[u8], masked: bool) -> Result<Vec<u8>, WebSocketError> { let mask=if masked{let mut key=[0u8;4];getrandom::fill(&mut key).map_err(|_|WebSocketError::Entropy)?;Some(key)}else{None};encode_frame(true,opcode,payload,mask) }
pub fn encode_frame(fin: bool, opcode: u8, payload: &[u8], mask: Option<[u8; 4]>) -> Result<Vec<u8>, WebSocketError> {
    validate_opcode(fin, opcode, payload)?; let mut output = Vec::with_capacity(payload.len() + 14); output.push((u8::from(fin) << 7) | opcode); let masked = mask.is_some();
    match payload.len() { length @ 0..=125 => output.push((u8::from(masked) << 7) | length as u8), length @ 126..=65535 => { output.push((u8::from(masked) << 7) | 126); output.extend_from_slice(&(length as u16).to_be_bytes()); } length => { output.push((u8::from(masked) << 7) | 127); output.extend_from_slice(&(length as u64).to_be_bytes()); } }
    if let Some(key) = mask { output.extend_from_slice(&key); output.extend(payload.iter().enumerate().map(|(index, byte)| byte ^ key[index % 4])); } else { output.extend_from_slice(payload); }
    Ok(output)
}
pub fn parse_frame(buffer: &[u8], require_mask: Option<bool>, max_payload: usize) -> Result<Option<Frame>, WebSocketError> {
    if buffer.len() < 2 { return Ok(None); } let first=buffer[0];let second=buffer[1];if first&0x70!=0{return Err(WebSocketError::ReservedBits)}let fin=first&0x80!=0;let opcode=first&0x0f;let masked=second&0x80!=0;if require_mask.is_some_and(|required|required!=masked){return Err(WebSocketError::Masking)}let marker=second&0x7f;let mut offset=2;
    let length=match marker { 0..=125=>marker as u64,126=>{if buffer.len()<4{return Ok(None)}offset=4;let value=u16::from_be_bytes([buffer[2],buffer[3]]) as u64;if value<126{return Err(WebSocketError::NonMinimalLength)}value},127=>{if buffer.len()<10{return Ok(None)}offset=10;let value=u64::from_be_bytes(buffer[2..10].try_into().unwrap());if value<65536||value>>63!=0{return Err(WebSocketError::NonMinimalLength)}value},_=>unreachable!()};let length=usize::try_from(length).map_err(|_|WebSocketError::TooLarge)?;if length>max_payload{return Err(WebSocketError::TooLarge)}
    let mask=if masked{if buffer.len()<offset+4{return Ok(None)}let key: [u8;4]=buffer[offset..offset+4].try_into().unwrap();offset+=4;Some(key)}else{None};let end=offset.checked_add(length).ok_or(WebSocketError::TooLarge)?;if buffer.len()<end{return Ok(None)}let mut payload=buffer[offset..end].to_vec();if let Some(key)=mask{for(index,byte)in payload.iter_mut().enumerate(){*byte^=key[index%4]}}validate_opcode(fin,opcode,&payload)?;Ok(Some(Frame{fin,opcode,payload,consumed:end}))
}
fn decode_data(opcode:u8,payload:Vec<u8>)->Result<Message,WebSocketError>{match opcode{1=>String::from_utf8(payload).map(Message::Text).map_err(|_|WebSocketError::InvalidUtf8),2=>Ok(Message::Binary(payload)),_=>Err(WebSocketError::Opcode(opcode))}}
fn decode_close(payload: Vec<u8>) -> Result<Message, WebSocketError> {
    if payload.is_empty() { return Ok(Message::Close { code: None, reason: String::new() }); }
    if payload.len() < 2 { return Err(WebSocketError::InvalidClose); }
    let code = u16::from_be_bytes([payload[0], payload[1]]);
    if !valid_close_code(code) { return Err(WebSocketError::InvalidClose); }
    let reason = std::str::from_utf8(&payload[2..]).map_err(|_| WebSocketError::InvalidClose)?.into();
    Ok(Message::Close { code: Some(code), reason })
}
fn valid_close_code(code:u16)->bool{matches!(code,1000..=1003|1007..=1014|3000..=4999)}
fn validate_opcode(fin: bool, opcode: u8, payload: &[u8]) -> Result<(), WebSocketError> {
    match opcode {
        0..=2 => {}
        8..=10 => {
            if !fin || payload.len() > 125 {
                return Err(WebSocketError::ControlFrame);
            }
            if opcode == 8 && payload.len() == 1 {
                return Err(WebSocketError::ControlFrame);
            }
        }
        _ => return Err(WebSocketError::Opcode(opcode)),
    }
    if opcode == 1 && fin && std::str::from_utf8(payload).is_err() {
        return Err(WebSocketError::InvalidUtf8);
    }
    Ok(())
}
fn valid_token(value:&str)->bool{!value.is_empty()&&value.bytes().all(|byte|byte.is_ascii_alphanumeric()||b"!#$%&'*+-.^_`|~".contains(&byte))}

#[cfg(test)] mod tests { use super::*; #[test] fn matches_rfc_handshake_vector(){assert_eq!(accept_key("dGhlIHNhbXBsZSBub25jZQ==").unwrap(),"s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");let response=String::from_utf8(upgrade_response("dGhlIHNhbXBsZSBub25jZQ==",Some("chat")).unwrap()).unwrap();assert!(response.contains("101 Switching Protocols"));assert!(response.contains("Sec-WebSocket-Protocol: chat"));}#[test]fn round_trips_masked_text_and_partial_frames(){let encoded=encode_frame(true,1,b"hello",Some([1,2,3,4])).unwrap();assert!(parse_frame(&encoded[..3],Some(true),1024).unwrap().is_none());let frame=parse_frame(&encoded,Some(true),1024).unwrap().unwrap();assert_eq!(frame.payload,b"hello");assert_eq!(frame.consumed,encoded.len());}#[test]fn rejects_protocol_violations(){assert_eq!(parse_frame(&[0x81,0x80],Some(false),1024),Err(WebSocketError::Masking));assert!(encode_frame(false,9,b"ping",None).is_err());assert!(encode_frame(true,1,&[0xff],None).is_err());}#[test]fn reassembles_fragmented_messages_with_interleaved_ping(){let mut decoder=MessageDecoder::new(1024);let mut bytes=encode_frame(false,1,b"hel",None).unwrap();bytes.extend(encode_frame(true,9,b"p",None).unwrap());bytes.extend(encode_frame(true,0,b"lo",None).unwrap());decoder.push(&bytes).unwrap();assert_eq!(decoder.next(Some(false)).unwrap(),Some(Message::Ping(b"p".to_vec())));assert_eq!(decoder.next(Some(false)).unwrap(),Some(Message::Text("hello".into())));}#[test]fn validates_close_codes_and_fragment_limits(){let mut decoder=MessageDecoder::new(4);decoder.push(&encode_frame(false,2,b"123",None).unwrap()).unwrap();assert_eq!(decoder.next(Some(false)).unwrap(),None);decoder.push(&encode_frame(true,0,b"45",None).unwrap()).unwrap();assert_eq!(decoder.next(Some(false)),Err(WebSocketError::TooLarge));let invalid=encode_frame(true,8,&999u16.to_be_bytes(),None).unwrap();let mut close_decoder=MessageDecoder::new(1024);close_decoder.push(&invalid).unwrap();assert_eq!(close_decoder.next(Some(false)),Err(WebSocketError::InvalidClose));}}
