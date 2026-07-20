use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock, atomic::{AtomicU64, Ordering}};
use std::time::{Duration, Instant};
use titan_stdlib::{self as stdlib, native::Capability};
use crate::{RuntimeCapabilities, Value, VmError, val_to_string};

pub fn invoke(name: &str, args: Vec<Value>, capabilities: RuntimeCapabilities) -> Result<Value, VmError> {
    let signature = stdlib::native::lookup(name).ok_or_else(|| failure(name, "function is not registered"))?;
    if args.len() != signature.params.len() { return Err(failure(name, &format!("expected {} arguments, found {}", signature.params.len(), args.len()))); }
    require_capability(name, signature.capability, capabilities)?;
    dispatch(name, args).map_err(|message| failure(name, &message))
}

fn dispatch(name: &str, mut args: Vec<Value>) -> Result<Value, String> {
    macro_rules! take { () => { args.remove(0) }; }
    macro_rules! string { () => { expect_string(take!())? }; }
    macro_rules! bytes { () => { expect_bytes(take!())? }; }
    macro_rules! int { () => { expect_int(take!())? }; }
    macro_rules! float { () => { expect_float(take!())? }; }
    macro_rules! array { () => { expect_array(take!())? }; }
    macro_rules! boolean { () => { expect_bool(take!())? }; }
    Ok(match name {
        "std::text::length" => Value::Int(to_i64(stdlib::text::length(&string!()))?),
        "std::text::reverse" => Value::Str(stdlib::text::reverse(&string!())),
        "std::text::uppercase" => Value::Str(string!().to_uppercase()),
        "std::text::lowercase" => Value::Str(string!().to_lowercase()),
        "std::text::trim" => Value::Str(string!().trim().into()),
        "std::text::capitalize" => Value::Str(stdlib::text::capitalize(&string!())),
        "std::text::escape_html" => Value::Str(stdlib::text::escape_html(&string!())),
        "std::text::slugify" => Value::Str(stdlib::text::slugify(&string!())),
        "std::text::levenshtein" => { let a = string!(); let b = string!(); Value::Int(to_i64(stdlib::text::levenshtein(&a, &b))?) }
        "std::text::equals" => Value::Bool(string!() == string!()),
        "std::text::hash64" => Value::Int(i64::from_ne_bytes(stdlib::checksum::fnv1a_64(string!().as_bytes()).to_ne_bytes())),
        "std::text::contains" => { let text = string!(); Value::Bool(text.contains(&string!())) }
        "std::text::starts_with" => { let text = string!(); Value::Bool(text.starts_with(&string!())) }
        "std::text::ends_with" => { let text = string!(); Value::Bool(text.ends_with(&string!())) }
        "std::text::replace" => { let text = string!(); let from = string!(); Value::Str(text.replace(&from, &string!())) }
        "std::text::truncate" => { let text = string!(); let max = nonnegative(int!())?; Value::Str(stdlib::text::truncate(&text, max, &string!())) }
        "std::text::words" => Value::Array(string!().split_whitespace().map(|v| Value::Str(v.into())).collect()),
        "std::text::lines" => Value::Array(string!().lines().map(|v| Value::Str(v.into())).collect()),

        "std::encoding::hex_encode" => Value::Str(stdlib::encoding::hex_encode(&bytes!())),
        "std::encoding::hex_decode" => Value::Bytes(stdlib::encoding::hex_decode(&string!()).map_err(error)?),
        "std::encoding::base64_encode" => Value::Str(stdlib::encoding::base64_encode(&bytes!())),
        "std::encoding::base64_decode" => Value::Bytes(stdlib::encoding::base64_decode(&string!()).map_err(error)?),
        "std::encoding::percent_encode" => Value::Str(stdlib::encoding::percent_encode(&string!())),
        "std::encoding::percent_decode" => Value::Str(stdlib::encoding::percent_decode(&string!()).map_err(error)?),
        "std::encoding::utf8_encode" => Value::Bytes(string!().into_bytes()),
        "std::encoding::utf8_decode" => Value::Str(String::from_utf8(bytes!()).map_err(error)?),

        "std::checksum::fnv1a64" => Value::Int(stdlib::checksum::fnv1a_64(&bytes!()) as i64),
        "std::checksum::crc32" => Value::Int(i64::from(stdlib::checksum::crc32(&bytes!()))),
        "std::checksum::constant_time_eq" => { let a = bytes!(); Value::Bool(stdlib::checksum::constant_time_eq(&a, &bytes!())) }
        "std::bytes::from_array" => Value::Bytes(array!().into_iter().map(|value| { let value = expect_int(value)?; u8::try_from(value).map_err(|_| "byte values must be between 0 and 255".into()) }).collect::<Result<Vec<_>, String>>()?),
        "std::bytes::to_array" => Value::Array(bytes!().into_iter().map(|value| Value::Int(i64::from(value))).collect()),
        "std::bytes::length" => Value::Int(to_i64(bytes!().len())?),
        "std::bytes::concat" => { let mut left = bytes!(); left.extend(bytes!()); Value::Bytes(left) }
        "std::bytes::slice" => { let values = bytes!(); let start = nonnegative(int!())?; let end = nonnegative(int!())?; if start > end || end > values.len() { return Err("invalid byte slice bounds".into()); } Value::Bytes(values[start..end].to_vec()) }
        "std::bytes::read_u32_le" => { let values = bytes!(); let offset = nonnegative(int!())?; let end = offset.checked_add(4).ok_or("byte offset overflow")?; let data: [u8; 4] = values.get(offset..end).ok_or("not enough bytes for u32")?.try_into().map_err(|_| "not enough bytes for u32")?; Value::Int(i64::from(u32::from_le_bytes(data))) }
        "std::bytes::write_u32_le" => { let value = u32::try_from(int!()).map_err(|_| "u32 value out of range")?; Value::Bytes(value.to_le_bytes().to_vec()) }

        "std::http::parse_request" => {
            let parsed = stdlib::http::parse_request(&bytes!(), &stdlib::http::HttpLimits::default()).map_err(error)?;
            if let Some((request, consumed)) = parsed { let mut map = BTreeMap::new(); map.insert("method".into(), Value::Str(request.method)); map.insert("target".into(), Value::Str(request.target)); map.insert("path".into(), Value::Str(request.path)); map.insert("query".into(), request.query.map(Value::Str).unwrap_or(Value::Nil)); map.insert("version".into(), Value::Str(request.version)); map.insert("headers".into(), Value::Map(request.headers.into_iter().map(|(key,value)|(key,Value::Str(value))).collect())); map.insert("body".into(), Value::Bytes(request.body)); map.insert("keep_alive".into(), Value::Bool(request.keep_alive)); map.insert("consumed".into(), Value::Int(consumed as i64)); Value::Enum { name:"Option".into(), variant:"Some".into(), payload:Some(Box::new(Value::Map(map))) } } else { Value::Enum { name:"Option".into(), variant:"None".into(), payload:None } }
        }
        "std::http::build_response" => { let status = u16::try_from(int!()).map_err(|_| "HTTP status out of range")?; let headers = expect_map(take!())?.into_iter().map(|(key,value)| Ok((key,expect_string(value)?))).collect::<Result<BTreeMap<_,_>,String>>()?; let body=bytes!(); let keep_alive=expect_bool(take!())?; Value::Bytes(stdlib::http::build_response(status,&headers,&body,keep_alive).map_err(error)?) }
        "std::http::reason_phrase" => { let status=u16::try_from(int!()).map_err(|_| "HTTP status out of range")?; stdlib::http::reason_phrase(status).map(|value|Value::Str(value.into())).unwrap_or(Value::Nil) }
        "std::http::route_match" => { let pattern=string!();let path=string!();match stdlib::http::match_route(&pattern,&path).map_err(error)? { Some(params)=>Value::Enum{name:"Option".into(),variant:"Some".into(),payload:Some(Box::new(Value::Map(params.into_iter().map(|(key,value)|(key,Value::Str(value))).collect())))},None=>Value::Enum{name:"Option".into(),variant:"None".into(),payload:None} } }
        "std::http::parse_query" => { let query=string!();let limit=nonnegative(int!())?;Value::Map(stdlib::http::parse_query(&query,limit).map_err(error)?.into_iter().map(|(key,values)|(key,Value::Array(values.into_iter().map(Value::Str).collect()))).collect()) }
        "std::http::security_headers" => with_response_headers(take!(), |headers| { headers.entry("X-Content-Type-Options".into()).or_insert_with(||Value::Str("nosniff".into()));headers.entry("X-Frame-Options".into()).or_insert_with(||Value::Str("DENY".into()));headers.entry("Referrer-Policy".into()).or_insert_with(||Value::Str("no-referrer".into()));headers.entry("Content-Security-Policy".into()).or_insert_with(||Value::Str("default-src 'none'; frame-ancestors 'none'".into()));Ok(()) })?,
        "std::http::cors" => { let response=take!();let origin=string!();let methods=string!();if origin.contains(['\r','\n'])||methods.contains(['\r','\n']){return Err("invalid CORS header value".into())}with_response_headers(response,|headers|{headers.insert("Access-Control-Allow-Origin".into(),Value::Str(origin));headers.insert("Access-Control-Allow-Methods".into(),Value::Str(methods));headers.insert("Vary".into(),Value::Str("Origin".into()));Ok(())})? }
        "std::http::request_id" => { let mut request=expect_map(take!())?;let id=REQUEST_IDS.fetch_add(1,Ordering::Relaxed);request.insert("request_id".into(),Value::Str(format!("titan-{id:016x}")));Value::Map(request) }
        "std::http::rate_limit" => { let key=string!();let maximum=u64::try_from(int!()).map_err(|_|"rate limit maximum must be nonnegative")?;let window=u64::try_from(int!()).map_err(|_|"rate limit window must be nonnegative")?;Value::Bool(rate_limit(&key,maximum,Duration::from_millis(window))?) }
        "std::http::json_response" => { let status=int!();let value=to_json(take!())?;http_response_map(status,"application/json; charset=utf-8",serde_json::to_vec(&value).map_err(error)?) }
        "std::http::error_response" => { let status=int!();let message=string!();let body=serde_json::to_vec(&serde_json::json!({"error":message})).map_err(error)?;http_response_map(status,"application/json; charset=utf-8",body) }
        "std::http::request" => { let method=string!();let url=string!();let headers=expect_map(take!())?.into_iter().map(|(key,value)|Ok((key,expect_string(value)?))).collect::<Result<BTreeMap<_,_>,String>>()?;let body=bytes!();let maximum=nonnegative(int!())?;let redirects=nonnegative(int!())?;let timeout=u64::try_from(int!()).map_err(|_|"HTTP timeout must be nonnegative")?;let response=stdlib::http_client::request(stdlib::http_client::Request{method,url,headers,body,maximum_body:maximum,redirects,timeout:Duration::from_millis(timeout)}).map_err(error)?;Value::Map(BTreeMap::from([("status".into(),Value::Int(response.status as i64)),("headers".into(),Value::Map(response.headers.into_iter().map(|(key,value)|(key,Value::Str(value))).collect())),("body".into(),Value::Bytes(response.body)),("final_url".into(),Value::Str(response.final_url))])) }
        "std::http::parse_multipart" => {let content_type=string!();let body=bytes!();let max_parts=nonnegative(int!())?;let max_part=nonnegative(int!())?;Value::Array(stdlib::multipart::parse(&content_type,&body,max_parts,max_part).map_err(error)?.into_iter().map(|part|Value::Map(BTreeMap::from([("name".into(),Value::Str(part.name)),("filename".into(),part.filename.map(Value::Str).unwrap_or(Value::Nil)),("content_type".into(),part.content_type.map(Value::Str).unwrap_or(Value::Nil)),("headers".into(),Value::Map(part.headers.into_iter().map(|(key,value)|(key,Value::Str(value))).collect())),("data".into(),Value::Bytes(part.data))]))).collect())}
        "std::metrics::counter_add" => {let name=string!();let amount=u64::try_from(int!()).map_err(|_|"counter amount must be nonnegative")?;Value::Int(i64::try_from(stdlib::metrics::counter_add(&name,amount).map_err(error)?).unwrap_or(i64::MAX))}
        "std::metrics::gauge_set" => {let name=string!();let value=float!();stdlib::metrics::gauge_set(&name,value).map_err(error)?;Value::Nil}
        "std::metrics::histogram_record" => {let name=string!();let value=float!();stdlib::metrics::histogram_record(&name,value).map_err(error)?;Value::Nil}
        "std::metrics::snapshot" => metrics_snapshot(stdlib::metrics::snapshot().map_err(error)?),
        "std::metrics::reset" => {stdlib::metrics::reset().map_err(error)?;Value::Nil}
        "std::ws::accept_key" => Value::Str(stdlib::websocket::accept_key(&string!()).map_err(error)?),
        "std::ws::upgrade_response" => {let key=string!();let protocol=string!();Value::Bytes(stdlib::websocket::upgrade_response(&key,if protocol.is_empty(){None}else{Some(&protocol)}).map_err(error)?)},
        "std::ws::validate_upgrade" => {let request=take!();let protocol=string!();Value::Bytes(websocket_upgrade(request,&protocol)?)},
        "std::ws::validate_accept" => {let response=bytes!();let key=string!();Value::Bool(websocket_validate_accept(&response,&key)?)},
        "std::ws::encode" => {let opcode=u8::try_from(int!()).map_err(|_|"WebSocket opcode out of range")?;let payload=bytes!();let masked=expect_bool(take!())?;Value::Bytes(stdlib::websocket::encode_frame_with_policy(opcode,&payload,masked).map_err(error)?)},
        "std::ws::parse" => {let data=bytes!();let require_mask=expect_bool(take!())?;let maximum=nonnegative(int!())?;match stdlib::websocket::parse_frame(&data,Some(require_mask),maximum).map_err(error)?{Some(frame)=>Value::Enum{name:"Option".into(),variant:"Some".into(),payload:Some(Box::new(Value::Map(BTreeMap::from([("fin".into(),Value::Bool(frame.fin)),("opcode".into(),Value::Int(frame.opcode as i64)),("payload".into(),Value::Bytes(frame.payload)),("consumed".into(),Value::Int(frame.consumed as i64))]))))},None=>Value::Enum{name:"Option".into(),variant:"None".into(),payload:None}}},
        "std::csv::parse" => Value::Array(stdlib::csv::parse(&string!()).map_err(error)?.into_iter().map(|row| Value::Array(row.into_iter().map(Value::Str).collect())).collect()),
        "std::csv::serialize" => { let rows = array!().into_iter().map(expect_string_array).collect::<Result<Vec<_>, _>>()?; Value::Str(stdlib::csv::serialize(&rows)) }
        "std::json::parse" => from_json(stdlib::json::parse(&string!()).map_err(error)?)?,
        "std::json::stringify" => Value::Str(stdlib::json::stringify(&to_json(take!())?)),
        "std::json::pretty" => Value::Str(stdlib::json::stringify_pretty(&to_json(take!())?).map_err(error)?),
        "std::json::pointer" => { let value = to_json(take!())?; let pointer = string!(); value.pointer(&pointer).cloned().map(from_json).transpose()?.unwrap_or(Value::Nil) }
        "std::json::merge" => { let mut target = to_json(take!())?; let patch = to_json(take!())?; stdlib::json::merge(&mut target, patch); from_json(target)? }
        "std::json::flatten" => Value::Array(stdlib::json::flatten(&to_json(take!())?).into_iter().map(|(path, value)| Ok(Value::Tuple(vec![Value::Str(path), from_json(value)?]))).collect::<Result<Vec<_>, String>>()?),

        "std::array::set" => { let mut values=array!();let index=nonnegative(int!())?;let value=take!();let Some(slot)=values.get_mut(index)else{return Err("array index out of bounds".into())};*slot=value;Value::Array(values) }
        "std::array::push" => { let mut values=array!();values.push(take!());Value::Array(values) }
        "std::array::pop" => { let mut values=array!();let _=values.pop();Value::Array(values) }
        "std::array::slice" => { let values=array!();let start=nonnegative(int!())?;let end=nonnegative(int!())?;if start>end||end>values.len(){return Err("invalid array slice range".into())}Value::Array(values[start..end].to_vec()) }
        "std::array::concat" => { let mut left=array!();left.extend(array!());Value::Array(left) }
        "std::collections::length" => Value::Int(to_i64(value_length(&take!())?)?),
        "std::collections::contains" => { let values = array!(); Value::Bool(values.contains(&take!())) }
        "std::collections::reverse" => { let mut values = array!(); values.reverse(); Value::Array(values) }
        "std::collections::deduplicate" => { let mut output = Vec::new(); for value in array!() { if !output.contains(&value) { output.push(value); } } Value::Array(output) }
        "std::collections::join" => { let values = array!(); let separator = string!(); Value::Str(values.iter().map(val_to_string).collect::<Vec<_>>().join(&separator)) }
        "std::collections::chunk" => { let values = array!(); let size = nonnegative(int!())?; if size == 0 { return Err("chunk size must be positive".into()); } Value::Array(values.chunks(size).map(|part| Value::Array(part.to_vec())).collect()) }
        "std::map::new" => Value::Map(BTreeMap::new()),
        "std::map::length" => Value::Int(to_i64(expect_map(take!())?.len())?),
        "std::map::insert_new" => { let mut values=expect_map(take!())?;let key=string!();if values.contains_key(&key){return Err("map key already exists".into())}values.insert(key,take!());Value::Map(values) }
        "std::map::keys" => Value::Array(expect_map(take!())?.into_keys().map(Value::Str).collect()),
        "std::map::values" => Value::Array(expect_map(take!())?.into_values().collect()),
        "std::map::contains" => { let values = expect_map(take!())?; Value::Bool(values.contains_key(&string!())) }
        "std::map::get" => { let values = expect_map(take!())?; values.get(&string!()).cloned().unwrap_or(Value::Nil) }
        "std::map::insert" => { let mut values = expect_map(take!())?; let key = string!(); values.insert(key, take!()); Value::Map(values) }
        "std::map::remove" => { let mut values = expect_map(take!())?; values.remove(&string!()); Value::Map(values) }

        "std::math::sqrt" => checked_float(float!().sqrt(), "sqrt domain error")?,
        "std::math::pow" => { let base = float!(); checked_float(base.powf(float!()), "pow domain error")? }
        "std::math::sin" => Value::Float(float!().sin()), "std::math::cos" => Value::Float(float!().cos()),
        "std::math::tan" => checked_float(float!().tan(), "tan produced a non-finite result")?,
        "std::math::ln" => checked_float(float!().ln(), "ln domain error")?,
        "std::math::abs" => Value::Float(float!().abs()), "std::math::floor" => Value::Float(float!().floor()),
        "std::math::ceil" => Value::Float(float!().ceil()), "std::math::round" => Value::Float(float!().round()),
        "std::stats::mean" => { let values = numbers(array!())?; if values.is_empty() { return Err("mean requires at least one number".into()); } Value::Float(values.iter().sum::<f64>() / values.len() as f64) }
        "std::stats::median" => { let mut values = numbers(array!())?; Value::Float(stdlib::stats::median(&mut values).ok_or("median requires finite numbers")?) }
        "std::stats::quantile" => { let mut values = numbers(array!())?; let q = float!(); Value::Float(stdlib::stats::quantile(&mut values, q).ok_or("invalid quantile or input")?) }
        "std::stats::variance" => { let values = numbers(array!())?; let mut summary = stdlib::stats::Summary::new(); summary.extend(values); Value::Float(summary.variance_population().ok_or("variance requires numbers")?) }
        "std::stats::stddev" => { let values = numbers(array!())?; let mut summary = stdlib::stats::Summary::new(); summary.extend(values); Value::Float(summary.standard_deviation().ok_or("standard deviation requires numbers")?) }

        "std::time::unix_seconds" => Value::Int(i64::try_from(stdlib::time::unix_seconds().map_err(error)?).map_err(error)?),
        "std::time::unix_millis" => Value::Int(i64::try_from(stdlib::time::unix_millis().map_err(error)?).map_err(error)?),
        "std::time::sleep_ms" => { stdlib::time::sleep(Duration::from_millis(u64::try_from(int!()).map_err(|_| "milliseconds must be nonnegative")?)); Value::Nil }

        "std::path::join" => Value::Str(stdlib::path::join(string!(), string!()).to_string_lossy().into()),
        "std::path::normalize" => Value::Str(stdlib::path::normalize(string!()).to_string_lossy().into()),
        "std::path::parent" => optional_path(stdlib::path::parent(string!())),
        "std::path::file_name" => optional_string(stdlib::path::file_name(string!())),
        "std::path::stem" => optional_string(stdlib::path::stem(string!())),
        "std::path::extension" => optional_string(stdlib::path::extension(string!())),
        "std::path::absolute" => Value::Str(stdlib::path::absolute(string!()).map_err(error)?.to_string_lossy().into()),
        "std::path::canonical" => Value::Str(stdlib::path::canonical(string!()).map_err(error)?.to_string_lossy().into()),

        "std::fs::read_text" => Value::Str(stdlib::io::read_file(string!()).map_err(error)?),
        "std::fs::read_bytes" => Value::Bytes(stdlib::io::read_bytes(string!()).map_err(error)?),
        "std::fs::write_text" => { let path = string!(); stdlib::io::write_file(path, &string!()).map_err(error)?; Value::Nil }
        "std::fs::write_bytes" => { let path = string!(); stdlib::io::write_bytes(path, &bytes!()).map_err(error)?; Value::Nil }
        "std::fs::atomic_write" => { let path = string!(); stdlib::io::atomic_write(path, &bytes!()).map_err(error)?; Value::Nil }
        "std::fs::append" => { let path = string!(); stdlib::io::append(path, &bytes!()).map_err(error)?; Value::Nil }
        "std::fs::exists" => Value::Bool(stdlib::io::exists(string!())), "std::fs::is_file" => Value::Bool(stdlib::io::is_file(string!())),
        "std::fs::is_dir" => Value::Bool(stdlib::io::is_dir(string!())),
        "std::fs::create_dir" => { stdlib::io::create_dir(string!()).map_err(error)?; Value::Nil }
        "std::fs::remove_file" => { stdlib::io::remove_file(string!()).map_err(error)?; Value::Nil }
        "std::fs::remove_dir" => { stdlib::io::remove_dir(string!()).map_err(error)?; Value::Nil }
        "std::fs::list_dir" => Value::Array(stdlib::io::list_dir(string!()).map_err(error)?.into_iter().map(|p| Value::Str(p.to_string_lossy().into())).collect()),
        "std::fs::file_size" => Value::Int(i64::try_from(stdlib::io::file_size(string!()).map_err(error)?).map_err(error)?),
        "std::fs::copy" => { let from = string!(); Value::Int(i64::try_from(stdlib::io::copy(from, string!()).map_err(error)?).map_err(error)?) }
        "std::fs::rename" => { let from = string!(); stdlib::io::rename(from, string!()).map_err(error)?; Value::Nil }

        "std::process::run" => { let program = string!(); process_output(stdlib::process::CommandSpec::new(program).args(strings(array!())?).output().map_err(error)?) }
        "std::process::run_timeout" => { let program = string!(); let arguments = strings(array!())?; let timeout = u64::try_from(int!()).map_err(|_| "timeout must be nonnegative")?; process_output(stdlib::process::CommandSpec::new(program).args(arguments).output_timeout(Duration::from_millis(timeout)).map_err(error)?) }
        "std::env::get" => std::env::var(string!()).map(Value::Str).unwrap_or(Value::Nil),
        "std::env::args" => Value::Array(std::env::args().map(Value::Str).collect()),
        "std::env::current_dir" => Value::Str(std::env::current_dir().map_err(error)?.to_string_lossy().into()),
        "std::net::http_get" => { let response = stdlib::net::http_get(&string!()).map_err(error)?; let mut map = BTreeMap::new(); map.insert("status".into(), Value::Int(i64::from(response.status))); map.insert("body".into(), Value::Bytes(response.body)); map.insert("headers".into(), Value::Array(response.headers.into_iter().map(|(k,v)| Value::Tuple(vec![Value::Str(k), Value::Str(v)])).collect())); Value::Map(map) }
        "std::web::query_exists" | "std::web::set_text" | "std::web::set_html" | "std::web::set_attribute" | "std::web::add_class" | "std::web::remove_class" | "std::web::focus" | "std::web::set_title" | "std::web::listen" | "std::web::unlisten" | "std::web::event_type" | "std::web::event_value" | "std::web::event_key" | "std::web::event_target_id" | "std::web::event_checked" | "std::web::event_x" | "std::web::event_y" | "std::web::fetch" | "std::web::fetch_cancel" | "std::web::fetch_ok" | "std::web::fetch_status" | "std::web::fetch_body" | "std::web::fetch_url" | "std::web::fetch_error" | "std::web::fetch_headers" | "std::web::request" | "std::web::ws_connect" | "std::web::ws_send" | "std::web::ws_close" | "std::web::ws_id" | "std::web::ws_message" | "std::web::ws_protocol" | "std::web::ws_close_code" | "std::web::ws_close_reason" | "std::web::ws_was_clean" | "std::web::ws_error" | "std::web::canvas_resize" | "std::web::canvas_clear" | "std::web::canvas_fill_rect" | "std::web::canvas_stroke_rect" | "std::web::canvas_line" | "std::web::canvas_text" | "std::web::animation_start" | "std::web::animation_cancel" | "std::web::frame_id" | "std::web::frame_time_ms" | "std::web::frame_delta_ms" | "std::web::frame_count" | "std::web::webgl_supported" | "std::web::webgl_create" | "std::web::webgl_uniform_f32" | "std::web::webgl_draw" | "std::web::webgl_delete" => return Err("std::web functions require the WebAssembly browser host".into()),
        "std::wasm::heap_used" | "std::wasm::heap_capacity" | "std::wasm::heap_limit" | "std::wasm::heap_set_limit" | "std::wasm::heap_checkpoint" | "std::wasm::heap_restore" | "std::wasm::heap_allocations" | "std::wasm::heap_allocated_bytes" | "std::wasm::heap_restores" | "std::wasm::heap_reclaimed_bytes" | "std::wasm::heap_peak_used" | "std::wasm::heap_reset_counters" | "std::wasm::heap_scope_begin" | "std::wasm::heap_scope_end" => return Err("std::wasm heap functions require the WebAssembly backend".into()),
        "std::window::create" => { let title = string!(); let w = int!() as u32; let h = int!() as u32; Value::Int(stdlib::window::create(&title, w, h) as i64) }
        "std::window::is_open" => { let id = int!() as u64; Value::Bool(stdlib::window::is_open(id)) }
        "std::window::close" => { let id = int!() as u64; Value::Bool(stdlib::window::close(id)) }
        "std::window::set_title" => { let id = int!() as u64; let title = string!(); Value::Bool(stdlib::window::set_title(id, &title)) }
        "std::window::resize" => { let id = int!() as u64; let w = int!() as u32; let h = int!() as u32; Value::Bool(stdlib::window::resize(id, w, h)) }
        "std::window::poll_events" => { let id = int!() as u64; Value::Array(stdlib::window::poll_events(id).into_iter().map(Value::Str).collect()) }

        "std::input::is_key_pressed" => { let key = string!(); Value::Bool(stdlib::input::is_key_pressed(&key)) }
        "std::input::mouse_pos" => { let (x, y) = stdlib::input::mouse_pos(); Value::Array(vec![Value::Int(i64::from(x)), Value::Int(i64::from(y))]) }
        "std::input::is_mouse_button_pressed" => { let btn = int!() as u8; Value::Bool(stdlib::input::is_mouse_button_pressed(btn)) }
        "std::input::touch_pos" => { let idx = int!() as u32; let (x, y, active) = stdlib::input::touch_pos(idx); Value::Array(vec![Value::Int(i64::from(x)), Value::Int(i64::from(y)), Value::Bool(active)]) }
        "std::clipboard::get_text" => Value::Str(stdlib::clipboard::get_text()),
        "std::clipboard::set_text" => { let text = string!(); Value::Bool(stdlib::clipboard::set_text(&text)) }
        "std::notify::send" => { let title = string!(); let body = string!(); Value::Bool(stdlib::clipboard::send_notification(&title, &body)) }
        "std::mobile::state" => Value::Str(stdlib::mobile::get_state()),
        "std::mobile::trigger" => { let event = string!(); Value::Bool(stdlib::mobile::trigger_event(&event)) }
        "std::mobile::poll_events" => Value::Array(stdlib::mobile::poll_events().into_iter().map(Value::Str).collect()),
        // Phase 8: Game & Audio
        "std::game::init" => { let title = string!(); let width = int!(); let height = int!(); Value::Bool(titan_game_init(&title, width, height)) }
        "std::game::step" => Value::Float(titan_game_step()),
        "std::game::fps" => Value::Int(titan_game_fps()),
        "std::game::check_collision" => {
            let x1 = float!(); let y1 = float!(); let w1 = float!(); let h1 = float!();
            let x2 = float!(); let y2 = float!(); let w2 = float!(); let h2 = float!();
            Value::Bool(titan_game_check_collision((x1, y1), (w1, h1), (x2, y2), (w2, h2)))
        }
        "std::audio::init" => Value::Bool(titan_audio_init()),
        "std::audio::load_wave" => { let freq_hz = float!(); let duration_ms = int!(); Value::Int(titan_audio_load_wave(freq_hz, duration_ms)) }
        "std::audio::sample_count" => { let handle = int!(); Value::Int(titan_audio_sample_count(handle) as i64) }
        "std::audio::play" => { let handle = int!(); let loop_audio = boolean!(); Value::Bool(titan_audio_play(handle, loop_audio)) }
        "std::audio::set_volume" => { let handle = int!(); let volume = float!(); Value::Bool(titan_audio_set_volume(handle, volume)) }
        "std::audio::stop" => { let handle = int!(); Value::Bool(titan_audio_stop(handle)) }
        // Phase 8: GUI Native Bindings
        "std::gui::init" => Value::Bool(titan_gui_init()),
        "std::gui::create_container" => { let title = string!(); let width = int!(); let height = int!(); Value::Int(titan_gui_create_container(&title, width, height)) }
        "std::gui::add_button" => { let parent = int!(); let label = string!(); let x = int!(); let y = int!(); let w = int!(); let h = int!(); Value::Int(titan_gui_add_button(parent, &label, x, y, w, h)) }
        "std::gui::add_label" => { let parent = int!(); let text = string!(); let x = int!(); let y = int!(); Value::Int(titan_gui_add_label(parent, &text, x, y)) }
        "std::gui::set_text" => { let id = int!(); let text = string!(); Value::Bool(titan_gui_set_text(id, &text)) }
        "std::gui::get_text" => { let id = int!(); Value::Str(titan_gui_get_text(id)) }
        "std::gui::trigger_click" => { let id = int!(); Value::Bool(titan_gui_trigger_click(id)) }
        "std::gui::is_clicked" => { let id = int!(); Value::Bool(titan_gui_is_clicked(id)) }
        "std::gui::child_count" => { let id = int!(); Value::Int(titan_gui_child_count(id) as i64) }
        "std::gui::shutdown" => Value::Bool(titan_gui_shutdown()),

        "std::testing::assert" => { let condition = take!(); let Value::Bool(condition) = condition else { return Err("assert condition must be bool".into()); }; let message = string!(); if !condition { return Err(format!("assertion failed: {message}")); } Value::Nil }
        "std::testing::assert_eq" => { let left = take!(); let right = take!(); let message = string!(); if left != right { return Err(format!("assertion failed: {message}; left={}, right={}", val_to_string(&left), val_to_string(&right))); } Value::Nil }
        _ => return Err("registered function has no VM implementation".into()),
    })
}

fn metrics_snapshot(snapshot:stdlib::metrics::Snapshot)->Value{let counters=snapshot.counters.into_iter().map(|(name,value)|(name,Value::Int(i64::try_from(value).unwrap_or(i64::MAX)))).collect();let gauges=snapshot.gauges.into_iter().map(|(name,value)|(name,Value::Float(value))).collect();let histograms=snapshot.histograms.into_iter().map(|(name,value)|(name,Value::Map(BTreeMap::from([("count".into(),Value::Int(i64::try_from(value.count).unwrap_or(i64::MAX))),("sum".into(),Value::Float(value.sum)),("min".into(),Value::Float(value.min)),("max".into(),Value::Float(value.max))])))).collect();Value::Map(BTreeMap::from([("counters".into(),Value::Map(counters)),("gauges".into(),Value::Map(gauges)),("histograms".into(),Value::Map(histograms))]))}
fn websocket_upgrade(request: Value, protocol: &str) -> Result<Vec<u8>, String> {
    let Value::Map(request) = request else { return Err("WebSocket upgrade request must be map".into()) };
    let method = match request.get("method") { Some(Value::Str(value)) => value, _ => return Err("WebSocket upgrade requires method".into()) };
    let version = match request.get("version") { Some(Value::Str(value)) => value, _ => return Err("WebSocket upgrade requires version".into()) };
    let headers = match request.get("headers") { Some(Value::Map(value)) => value, _ => return Err("WebSocket upgrade requires headers".into()) };
    if method != "GET" || version != "HTTP/1.1" { return Err("WebSocket upgrade requires GET HTTP/1.1".into()); }
    let header = |name: &str| headers.get(name).and_then(|value| if let Value::Str(value) = value { Some(value.as_str()) } else { None }).ok_or_else(|| format!("missing WebSocket header {name}"));
    let upgrade = header("upgrade")?; let connection = header("connection")?; let ws_version = header("sec-websocket-version")?; let key = header("sec-websocket-key")?;
    if !upgrade.eq_ignore_ascii_case("websocket") || !connection.split(',').any(|value| value.trim().eq_ignore_ascii_case("upgrade")) || ws_version != "13" { return Err("invalid WebSocket upgrade headers".into()); }
    if !protocol.is_empty() { let offered = header("sec-websocket-protocol")?; if !offered.split(',').any(|value| value.trim() == protocol) { return Err("selected WebSocket protocol was not offered".into()); } }
    stdlib::websocket::upgrade_response(key, if protocol.is_empty() { None } else { Some(protocol) }).map_err(error)
}
fn websocket_validate_accept(response:&[u8],key:&str)->Result<bool,String>{let Some(end)=response.windows(4).position(|window|window==b"\r\n\r\n")else{return Ok(false)};let text=std::str::from_utf8(&response[..end]).map_err(error)?;let mut lines=text.split("\r\n");if lines.next()!=Some("HTTP/1.1 101 Switching Protocols"){return Ok(false)}let mut headers:BTreeMap<String,Vec<String>>=BTreeMap::new();for line in lines{let Some((name,value))=line.split_once(':')else{return Ok(false)};headers.entry(name.to_ascii_lowercase()).or_default().push(value.trim().into());}let single=|name:&str|headers.get(name).filter(|values|values.len()==1).map(|values|values[0].as_str());let expected=stdlib::websocket::accept_key(key).map_err(error)?;Ok(single("upgrade").is_some_and(|value|value.eq_ignore_ascii_case("websocket"))&&single("connection").is_some_and(|value|value.split(',').any(|token|token.trim().eq_ignore_ascii_case("upgrade")))&&single("sec-websocket-accept")==Some(expected.as_str()))}
fn http_response_map(status:i64,content_type:&str,body:Vec<u8>)->Value{Value::Map(BTreeMap::from([("status".into(),Value::Int(status)),("headers".into(),Value::Map(BTreeMap::from([("Content-Type".into(),Value::Str(content_type.into()))]))),("body".into(),Value::Bytes(body)),("keep_alive".into(),Value::Bool(true))]))}
static REQUEST_IDS: AtomicU64 = AtomicU64::new(1);
static RATE_LIMITS: OnceLock<Mutex<HashMap<String, (Instant, u64)>>> = OnceLock::new();
fn with_response_headers(mut response: Value, update: impl FnOnce(&mut BTreeMap<String, Value>) -> Result<(), String>) -> Result<Value, String> { let Value::Map(response_map)=&mut response else{return Err("HTTP response must be map".into())};let headers=response_map.entry("headers".into()).or_insert_with(||Value::Map(BTreeMap::new()));let Value::Map(headers)=headers else{return Err("HTTP response headers must be map".into())};update(headers)?;Ok(response) }
fn rate_limit(key: &str, maximum: u64, window: Duration) -> Result<bool, String> {
    if maximum == 0 || window.is_zero() { return Ok(false); }
    let now = Instant::now();
    let mut limits = RATE_LIMITS.get_or_init(|| Mutex::new(HashMap::new())).lock().map_err(|_| "rate limit registry poisoned")?;
    let entry = limits.entry(key.into()).or_insert((now, 0));
    if now.duration_since(entry.0) >= window { *entry = (now, 0); }
    if entry.1 >= maximum { return Ok(false); }
    entry.1 += 1;
    Ok(true)
}
fn require_capability(name: &str, capability: Capability, caps: RuntimeCapabilities) -> Result<(), VmError> {
    let allowed = match capability { Capability::None => true, Capability::Filesystem => caps.filesystem, Capability::Process => caps.process, Capability::Network => caps.network, Capability::Environment => caps.environment };
    if allowed { Ok(()) } else { Err(VmError::PermissionDenied { function: name.into(), capability: format!("{capability:?}") }) }
}
fn failure(function: &str, message: &str) -> VmError { VmError::Native { function: function.into(), message: message.into() } }
fn error(error: impl std::fmt::Display) -> String { error.to_string() }
fn expect_string(value: Value) -> Result<String, String> { if let Value::Str(v) = value { Ok(v) } else { Err("expected string".into()) } }
fn expect_bytes(value: Value) -> Result<Vec<u8>, String> { match value { Value::Bytes(v) => Ok(v), Value::Str(v) => Ok(v.into_bytes()), _ => Err("expected bytes or string".into()) } }
fn expect_int(value: Value) -> Result<i64, String> { if let Value::Int(v) = value { Ok(v) } else { Err("expected int".into()) } }
fn expect_bool(value: Value) -> Result<bool, String> { if let Value::Bool(v) = value { Ok(v) } else { Err("expected bool".into()) } }
fn expect_float(value: Value) -> Result<f64, String> { match value { Value::Float(v) => Ok(v), Value::Int(v) => Ok(v as f64), _ => Err("expected number".into()) } }
fn expect_array(value: Value) -> Result<Vec<Value>, String> { match value { Value::Array(v) | Value::Tuple(v) => Ok(v), _ => Err("expected array".into()) } }
fn expect_map(value: Value) -> Result<BTreeMap<String, Value>, String> { if let Value::Map(values) = value { Ok(values) } else { Err("expected map".into()) } }
fn expect_string_array(value: Value) -> Result<Vec<String>, String> { expect_array(value)?.into_iter().map(expect_string).collect() }
fn strings(values: Vec<Value>) -> Result<Vec<String>, String> { values.into_iter().map(expect_string).collect() }
fn numbers(values: Vec<Value>) -> Result<Vec<f64>, String> { values.into_iter().map(expect_float).collect() }
fn nonnegative(value: i64) -> Result<usize, String> { usize::try_from(value).map_err(|_| "expected nonnegative integer".into()) }
fn to_i64(value: usize) -> Result<i64, String> { i64::try_from(value).map_err(error) }
fn checked_float(value: f64, message: &str) -> Result<Value, String> { if value.is_finite() { Ok(Value::Float(value)) } else { Err(message.into()) } }
fn optional_string(value: Option<String>) -> Value { value.map(Value::Str).unwrap_or(Value::Nil) }
fn optional_path(value: Option<std::path::PathBuf>) -> Value { value.map(|p| Value::Str(p.to_string_lossy().into())).unwrap_or(Value::Nil) }
fn value_length(value: &Value) -> Result<usize, String> { match value { Value::Str(v) => Ok(v.chars().count()), Value::Bytes(v) => Ok(v.len()), Value::Array(v) | Value::Tuple(v) => Ok(v.len()), Value::Map(v) => Ok(v.len()), _ => Err("value has no length".into()) } }
fn process_output(output: stdlib::process::ProcessOutput) -> Value { let mut map = BTreeMap::new(); map.insert("status".into(), output.status.map(|v| Value::Int(i64::from(v))).unwrap_or(Value::Nil)); map.insert("success".into(), Value::Bool(output.success)); map.insert("stdout".into(), Value::Bytes(output.stdout)); map.insert("stderr".into(), Value::Bytes(output.stderr)); map.insert("timed_out".into(), Value::Bool(output.timed_out)); Value::Map(map) }

fn to_json(value: Value) -> Result<serde_json::Value, String> {
    Ok(match value {
        Value::Nil => serde_json::Value::Null, Value::Bool(v) => v.into(), Value::Int(v) => v.into(),
        Value::Float(v) => serde_json::Number::from_f64(v).map(serde_json::Value::Number).ok_or("non-finite JSON number")?,
        Value::Char(v) => v.to_string().into(), Value::Str(v) => v.into(),
        Value::Bytes(v) => serde_json::Value::Array(v.into_iter().map(|x| serde_json::Value::from(u64::from(x))).collect()),
        Value::Array(v) | Value::Tuple(v) => serde_json::Value::Array(v.into_iter().map(to_json).collect::<Result<_, _>>()?),
        Value::Map(v) | Value::Struct { fields: v, .. } => serde_json::Value::Object(v.into_iter().map(|(k,v)| Ok((k, to_json(v)?))).collect::<Result<_, String>>()?),
        Value::Enum { name, variant, payload } => { let mut map = serde_json::Map::new(); map.insert("type".into(), name.into()); map.insert("variant".into(), variant.into()); if let Some(value) = payload { map.insert("payload".into(), to_json(*value)?); } serde_json::Value::Object(map) }
        Value::Closure { .. } | Value::Task(_) | Value::ChannelSender(_) | Value::ChannelReceiver(_) | Value::TcpListener(_) | Value::TcpStream(_) | Value::HttpRouter(_) | Value::TlsStream(_) | Value::TlsServerConfig(_) | Value::WebSocketDecoder(_) | Value::WebSocket(_) | Value::ServerControl(_) | Value::Sqlite(_) | Value::SqlitePool(_) | Value::Postgres(_) | Value::PostgresPool(_) | Value::Mysql(_) | Value::MysqlPool(_) => return Err("runtime handles cannot be encoded as JSON".into()),
    })
}
fn from_json(value: serde_json::Value) -> Result<Value, String> {
    Ok(match value {
        serde_json::Value::Null => Value::Nil, serde_json::Value::Bool(v) => Value::Bool(v),
        serde_json::Value::Number(v) => if let Some(i) = v.as_i64() { Value::Int(i) } else { Value::Float(v.as_f64().ok_or("invalid JSON number")?) },
        serde_json::Value::String(v) => Value::Str(v),
        serde_json::Value::Array(v) => Value::Array(v.into_iter().map(from_json).collect::<Result<_, _>>()?),
        serde_json::Value::Object(v) => Value::Map(v.into_iter().map(|(k,v)| Ok((k, from_json(v)?))).collect::<Result<_, String>>()?),
    })
}


// --- Phase 8: Game Loop & Audio Native Bindings ---
pub fn titan_game_init(title: &str, width: i64, height: i64) -> bool {
    titan_stdlib::game::init(title, width, height)
}
pub fn titan_game_step() -> f64 {
    titan_stdlib::game::step()
}
pub fn titan_game_fps() -> i64 {
    titan_stdlib::game::fps()
}
pub fn titan_game_check_collision(pos1: (f64, f64), size1: (f64, f64), pos2: (f64, f64), size2: (f64, f64)) -> bool {
    titan_stdlib::game::check_collision(pos1, size1, pos2, size2)
}
pub fn titan_audio_init() -> bool {
    titan_stdlib::audio::init()
}
pub fn titan_audio_load_wave(freq_hz: f64, duration_ms: i64) -> i64 {
    titan_stdlib::audio::load_wave(freq_hz, duration_ms)
}
pub fn titan_audio_sample_count(handle: i64) -> usize {
    titan_stdlib::audio::sample_count(handle)
}
pub fn titan_audio_play(handle: i64, loop_audio: bool) -> bool {
    titan_stdlib::audio::play(handle, loop_audio)
}
pub fn titan_audio_set_volume(handle: i64, volume: f64) -> bool {
    titan_stdlib::audio::set_volume(handle, volume)
}
pub fn titan_audio_stop(handle: i64) -> bool {
    titan_stdlib::audio::stop(handle)
}

// --- Phase 8: GUI Native Bindings ---
pub fn titan_gui_init() -> bool {
    titan_stdlib::gui::init()
}
pub fn titan_gui_create_container(title: &str, width: i64, height: i64) -> i64 {
    titan_stdlib::gui::create_container(title, width, height)
}
pub fn titan_gui_add_button(parent_id: i64, label: &str, x: i64, y: i64, width: i64, height: i64) -> i64 {
    titan_stdlib::gui::add_button(parent_id, label, x, y, width, height)
}
pub fn titan_gui_add_label(parent_id: i64, text: &str, x: i64, y: i64) -> i64 {
    titan_stdlib::gui::add_label(parent_id, text, x, y)
}
pub fn titan_gui_set_text(widget_id: i64, new_text: &str) -> bool {
    titan_stdlib::gui::set_text(widget_id, new_text)
}
pub fn titan_gui_get_text(widget_id: i64) -> String {
    titan_stdlib::gui::get_text(widget_id)
}
pub fn titan_gui_trigger_click(widget_id: i64) -> bool {
    titan_stdlib::gui::trigger_click(widget_id)
}
pub fn titan_gui_is_clicked(widget_id: i64) -> bool {
    titan_stdlib::gui::is_clicked(widget_id)
}
pub fn titan_gui_child_count(parent_id: i64) -> usize {
    titan_stdlib::gui::child_count(parent_id)
}
pub fn titan_gui_shutdown() -> bool {
    titan_stdlib::gui::shutdown()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_native_bindings() {
        let win_id = invoke("std::window::create", vec![Value::Str("VM Window".into()), Value::Int(800), Value::Int(600)], RuntimeCapabilities::all()).unwrap();
        assert!(matches!(win_id, Value::Int(i) if i > 0));
        if let Value::Int(id) = win_id {
            let is_open = invoke("std::window::is_open", vec![Value::Int(id)], RuntimeCapabilities::all()).unwrap();
            assert_eq!(is_open, Value::Bool(true));
            let closed = invoke("std::window::close", vec![Value::Int(id)], RuntimeCapabilities::all()).unwrap();
            assert_eq!(closed, Value::Bool(true));
        }
    }

    #[test]
    fn test_input_clipboard_native_bindings() {
        stdlib::input::set_key_state("Enter", true);
        let pressed = invoke("std::input::is_key_pressed", vec![Value::Str("Enter".into())], RuntimeCapabilities::all()).unwrap();
        assert_eq!(pressed, Value::Bool(true));

        invoke("std::clipboard::set_text", vec![Value::Str("Copied Data".into())], RuntimeCapabilities::all()).unwrap();
        let clip = invoke("std::clipboard::get_text", vec![], RuntimeCapabilities::all()).unwrap();
        assert_eq!(clip, Value::Str("Copied Data".into()));

        let notified = invoke("std::notify::send", vec![Value::Str("Alert".into()), Value::Str("Done".into())], RuntimeCapabilities::all()).unwrap();
        assert_eq!(notified, Value::Bool(true));
    }

    #[test]
    fn test_mobile_native_bindings() {
        invoke("std::mobile::trigger", vec![Value::Str("onPause".into())], RuntimeCapabilities::all()).unwrap();
        let state = invoke("std::mobile::state", vec![], RuntimeCapabilities::all()).unwrap();
        assert_eq!(state, Value::Str("Paused".into()));

        let events = invoke("std::mobile::poll_events", vec![], RuntimeCapabilities::all()).unwrap();
        assert!(matches!(events, Value::Array(v) if !v.is_empty()));
    }

    #[test]
    fn test_game_audio_native_bindings() {
        let init_game = invoke("std::game::init", vec![Value::Str("VM Game".into()), Value::Int(800), Value::Int(600)], RuntimeCapabilities::all()).unwrap();
        assert_eq!(init_game, Value::Bool(true));
        
        let _ = invoke("std::game::step", vec![], RuntimeCapabilities::all()).unwrap();
        let _ = invoke("std::game::fps", vec![], RuntimeCapabilities::all()).unwrap();
        
        let coll = invoke("std::game::check_collision", vec![
            Value::Float(0.0), Value::Float(0.0), Value::Float(10.0), Value::Float(10.0),
            Value::Float(2.0), Value::Float(2.0), Value::Float(10.0), Value::Float(10.0)
        ], RuntimeCapabilities::all()).unwrap();
        assert_eq!(coll, Value::Bool(true));

        assert_eq!(invoke("std::audio::init", vec![], RuntimeCapabilities::all()).unwrap(), Value::Bool(true));
        let handle_val = invoke("std::audio::load_wave", vec![Value::Float(220.0), Value::Int(50)], RuntimeCapabilities::all()).unwrap();
        if let Value::Int(handle) = handle_val {
            let _ = invoke("std::audio::sample_count", vec![Value::Int(handle)], RuntimeCapabilities::all()).unwrap();
            let _ = invoke("std::audio::play", vec![Value::Int(handle), Value::Bool(true)], RuntimeCapabilities::all()).unwrap();
            let _ = invoke("std::audio::set_volume", vec![Value::Int(handle), Value::Float(0.8)], RuntimeCapabilities::all()).unwrap();
            let _ = invoke("std::audio::stop", vec![Value::Int(handle)], RuntimeCapabilities::all()).unwrap();
        }
    }
    #[test]
    fn test_gui_native_bindings() {
        assert_eq!(invoke("std::gui::init", vec![], RuntimeCapabilities::all()).unwrap(), Value::Bool(true));
        
        let root = invoke("std::gui::create_container", vec![
            Value::Str("VM App".into()), Value::Int(1024), Value::Int(768)
        ], RuntimeCapabilities::all()).unwrap();
        
        if let Value::Int(root_id) = root {
            assert!(root_id > 0);
            let btn = invoke("std::gui::add_button", vec![
                Value::Int(root_id), Value::Str("Submit".into()), Value::Int(20), Value::Int(20), Value::Int(150), Value::Int(45)
            ], RuntimeCapabilities::all()).unwrap();
            
            if let Value::Int(btn_id) = btn {
                assert!(btn_id > 0);
                assert_eq!(invoke("std::gui::child_count", vec![Value::Int(root_id)], RuntimeCapabilities::all()).unwrap(), Value::Int(1));
                
                assert_eq!(invoke("std::gui::set_text", vec![Value::Int(btn_id), Value::Str("Send".into())], RuntimeCapabilities::all()).unwrap(), Value::Bool(true));
                assert_eq!(invoke("std::gui::get_text", vec![Value::Int(btn_id)], RuntimeCapabilities::all()).unwrap(), Value::Str("Send".into()));
                
                assert_eq!(invoke("std::gui::is_clicked", vec![Value::Int(btn_id)], RuntimeCapabilities::all()).unwrap(), Value::Bool(false));
                assert_eq!(invoke("std::gui::trigger_click", vec![Value::Int(btn_id)], RuntimeCapabilities::all()).unwrap(), Value::Bool(true));
                assert_eq!(invoke("std::gui::is_clicked", vec![Value::Int(btn_id)], RuntimeCapabilities::all()).unwrap(), Value::Bool(true));
            } else {
                panic!("add_button should return Int handle");
            }
            assert_eq!(invoke("std::gui::shutdown", vec![], RuntimeCapabilities::all()).unwrap(), Value::Bool(true));
        } else {
            panic!("create_container should return Int handle");
        }
    }
}
