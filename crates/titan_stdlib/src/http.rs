//! Defensive HTTP/1.1 message codec.

use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone)] pub struct HttpLimits { pub request_line: usize, pub headers_bytes: usize, pub header_count: usize, pub body_bytes: usize }
impl Default for HttpLimits { fn default() -> Self { Self { request_line: 8 * 1024, headers_bytes: 64 * 1024, header_count: 100, body_bytes: 16 * 1024 * 1024 } } }
#[derive(Debug, Clone, PartialEq, Eq)] pub struct Request { pub method: String, pub target: String, pub path: String, pub query: Option<String>, pub version: String, pub headers: BTreeMap<String, String>, pub body: Vec<u8>, pub keep_alive: bool }
#[derive(Error, Debug, Clone, PartialEq, Eq)] pub enum HttpError {
    #[error("HTTP headers exceed configured limit")] HeadersTooLarge,
    #[error("HTTP body exceeds configured limit")] BodyTooLarge,
    #[error("malformed HTTP request line")] RequestLine,
    #[error("unsupported HTTP version '{0}'")] Version(String),
    #[error("too many HTTP headers")] TooManyHeaders,
    #[error("malformed HTTP header")] Header,
    #[error("conflicting Content-Length headers")] ConflictingLength,
    #[error("invalid Content-Length")] InvalidLength,
    #[error("Transfer-Encoding is not accepted with this codec")] TransferEncoding,
    #[error("HTTP/1.1 requires exactly one Host header")] Host,
    #[error("invalid response status {0}")] InvalidStatus(u16),
    #[error("invalid response header name or value")] InvalidResponseHeader,
    #[error("invalid route pattern")] RoutePattern,
    #[error("invalid percent encoding in route/query")] RouteEncoding,
    #[error("query pair count exceeds configured limit")] TooManyQueryPairs,
}

pub fn parse_request(buffer: &[u8], limits: &HttpLimits) -> Result<Option<(Request, usize)>, HttpError> {
    let Some(header_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") else { if buffer.len() > limits.headers_bytes { return Err(HttpError::HeadersTooLarge); } return Ok(None) };
    if header_end + 4 > limits.headers_bytes { return Err(HttpError::HeadersTooLarge); }
    let header_text = std::str::from_utf8(&buffer[..header_end]).map_err(|_| HttpError::Header)?;
    let mut lines = header_text.split("\r\n"); let request_line = lines.next().ok_or(HttpError::RequestLine)?;
    if request_line.len() > limits.request_line { return Err(HttpError::RequestLine); }
    let mut parts = request_line.split(' '); let method = parts.next().filter(|value| valid_token(value)).ok_or(HttpError::RequestLine)?; let target = parts.next().filter(|value| !value.is_empty() && !value.bytes().any(|byte| byte.is_ascii_control())).ok_or(HttpError::RequestLine)?; let version = parts.next().ok_or(HttpError::RequestLine)?;
    if parts.next().is_some() { return Err(HttpError::RequestLine); }
    if version != "HTTP/1.1" && version != "HTTP/1.0" { return Err(HttpError::Version(version.into())); }
    let mut headers = BTreeMap::new(); let mut lengths = Vec::new(); let mut host_count = 0; let mut count = 0;
    for line in lines {
        count += 1;
        if count > limits.header_count { return Err(HttpError::TooManyHeaders); }
        if line.starts_with([' ', '\t']) { return Err(HttpError::Header); }
        let (name, value) = line.split_once(':').ok_or(HttpError::Header)?;
        if !valid_token(name) || value.contains(['\r','\n']) { return Err(HttpError::Header); }
        let name = name.to_ascii_lowercase(); let value = value.trim();
        if name == "content-length" { lengths.push(value.parse::<usize>().map_err(|_| HttpError::InvalidLength)?); }
        if name == "transfer-encoding" { return Err(HttpError::TransferEncoding); }
        if name == "host" { host_count += 1; }
        headers.entry(name).and_modify(|old: &mut String| { old.push_str(", "); old.push_str(value); }).or_insert_with(|| value.into());
    }
    if lengths.windows(2).any(|pair| pair[0] != pair[1]) { return Err(HttpError::ConflictingLength); }
    if version == "HTTP/1.1" && host_count != 1 { return Err(HttpError::Host); }
    let body_length = lengths.first().copied().unwrap_or(0); if body_length > limits.body_bytes { return Err(HttpError::BodyTooLarge); }
    let consumed = header_end + 4 + body_length; if buffer.len() < consumed { return Ok(None); }
    let (path, query) = target.split_once('?').map(|(path, query)| (path, Some(query.into()))).unwrap_or((target, None));
    let connection = headers.get("connection").map(|value| value.to_ascii_lowercase()); let keep_alive = if version == "HTTP/1.1" { connection.as_deref() != Some("close") } else { connection.as_deref() == Some("keep-alive") };
    Ok(Some((Request { method: method.into(), target: target.into(), path: path.into(), query, version: version.into(), headers, body: buffer[header_end + 4..consumed].to_vec(), keep_alive }, consumed)))
}

pub fn build_response(status: u16, headers: &BTreeMap<String, String>, body: &[u8], keep_alive: bool) -> Result<Vec<u8>, HttpError> {
    let reason = reason_phrase(status).ok_or(HttpError::InvalidStatus(status))?; let mut output = format!("HTTP/1.1 {status} {reason}\r\n").into_bytes();
    for (name, value) in headers { if !valid_token(name) || value.contains(['\r','\n']) || name.eq_ignore_ascii_case("content-length") || name.eq_ignore_ascii_case("connection") { return Err(HttpError::InvalidResponseHeader); } output.extend_from_slice(format!("{name}: {value}\r\n").as_bytes()); }
    output.extend_from_slice(format!("Content-Length: {}\r\nConnection: {}\r\n\r\n", body.len(), if keep_alive { "keep-alive" } else { "close" }).as_bytes()); output.extend_from_slice(body); Ok(output)
}
pub fn match_route(pattern: &str, path: &str) -> Result<Option<BTreeMap<String, String>>, HttpError> {
    let pattern_parts: Vec<_> = pattern.trim_matches('/').split('/').filter(|part| !part.is_empty()).collect();
    let path_parts: Vec<_> = path.trim_matches('/').split('/').filter(|part| !part.is_empty()).collect();
    let mut params = BTreeMap::new(); let mut path_index = 0;
    for (index, part) in pattern_parts.iter().enumerate() {
        if let Some(name) = part.strip_prefix('*') { if name.is_empty() || index + 1 != pattern_parts.len() { return Err(HttpError::RoutePattern); } let rest = path_parts[path_index..].join("/"); params.insert(name.into(), crate::encoding::percent_decode(&rest).map_err(|_| HttpError::RouteEncoding)?); path_index = path_parts.len(); break; }
        let Some(value) = path_parts.get(path_index) else { return Ok(None) };
        if let Some(name) = part.strip_prefix(':') { if name.is_empty() || params.contains_key(name) { return Err(HttpError::RoutePattern); } params.insert(name.into(), crate::encoding::percent_decode(value).map_err(|_| HttpError::RouteEncoding)?); }
        else if *part != *value { return Ok(None); }
        path_index += 1;
    }
    Ok((path_index == path_parts.len()).then_some(params))
}

pub fn parse_query(query: &str, max_pairs: usize) -> Result<BTreeMap<String, Vec<String>>, HttpError> {
    let mut output = BTreeMap::new(); if query.is_empty() { return Ok(output); }
    for (index, pair) in query.split('&').enumerate() { if index >= max_pairs { return Err(HttpError::TooManyQueryPairs); } let (key, value) = pair.split_once('=').unwrap_or((pair, "")); let key = crate::encoding::percent_decode(&key.replace('+', " ")).map_err(|_| HttpError::RouteEncoding)?; let value = crate::encoding::percent_decode(&value.replace('+', " ")).map_err(|_| HttpError::RouteEncoding)?; output.entry(key).or_insert_with(Vec::new).push(value); }
    Ok(output)
}

fn valid_token(value: &str) -> bool { !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)) }
pub fn reason_phrase(status: u16) -> Option<&'static str> { Some(match status { 100=>"Continue",101=>"Switching Protocols",200=>"OK",201=>"Created",202=>"Accepted",204=>"No Content",206=>"Partial Content",301=>"Moved Permanently",302=>"Found",304=>"Not Modified",307=>"Temporary Redirect",308=>"Permanent Redirect",400=>"Bad Request",401=>"Unauthorized",403=>"Forbidden",404=>"Not Found",405=>"Method Not Allowed",408=>"Request Timeout",409=>"Conflict",413=>"Content Too Large",415=>"Unsupported Media Type",422=>"Unprocessable Content",426=>"Upgrade Required",429=>"Too Many Requests",500=>"Internal Server Error",501=>"Not Implemented",502=>"Bad Gateway",503=>"Service Unavailable",504=>"Gateway Timeout",_=>return None }) }

#[cfg(test)] mod tests { use super::*; #[test] fn matches_routes_and_multivalue_queries() { let params=match_route("/users/:id/files/*path","/users/42/files/docs/readme.txt").unwrap().unwrap();assert_eq!(params["id"],"42");assert_eq!(params["path"],"docs/readme.txt");let query=parse_query("tag=rust&tag=titan&q=hello+world",10).unwrap();assert_eq!(query["tag"],vec!["rust","titan"]);assert_eq!(query["q"],vec!["hello world"]);assert!(match_route("/users/:id","/posts/1").unwrap().is_none()); }
#[test] fn rejects_bad_route_patterns_and_query_limits() { assert_eq!(match_route("/files/*path/more","/files/a/more"),Err(HttpError::RoutePattern));assert_eq!(parse_query("a=1&b=2",1),Err(HttpError::TooManyQueryPairs)); }
#[test] fn parses_incremental_keep_alive_request() { let request=b"POST /users?id=7 HTTP/1.1\r\nHost: localhost\r\nContent-Length: 4\r\n\r\ndataNEXT"; let (parsed,used)=parse_request(request,&HttpLimits::default()).unwrap().unwrap();assert_eq!(parsed.path,"/users");assert_eq!(parsed.query.as_deref(),Some("id=7"));assert_eq!(parsed.body,b"data");assert!(parsed.keep_alive);assert_eq!(&request[used..],b"NEXT"); } #[test] fn rejects_smuggling_and_injection() { assert_eq!(parse_request(b"POST / HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 2\r\n\r\na",&HttpLimits::default()),Err(HttpError::ConflictingLength)); let mut headers=BTreeMap::new();headers.insert("X-Test".into(),"ok\r\nInjected: yes".into());assert!(build_response(200,&headers,b"",false).is_err()); } #[test] fn builds_complete_response() { let response=build_response(200,&BTreeMap::from([("Content-Type".into(),"text/plain".into())]),b"hello",true).unwrap();let text=String::from_utf8(response).unwrap();assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));assert!(text.contains("Content-Length: 5"));assert!(text.ends_with("hello")); } }
