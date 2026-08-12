use crate::{val_to_string, RuntimeCapabilities, Value, VmError};
use std::collections::{BTreeMap, HashMap};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex, OnceLock,
};
use std::time::{Duration, Instant};
use titan_stdlib::{self as stdlib, native::Capability};

#[cfg(test)]
fn invoke(
    name: &str,
    args: Vec<Value>,
    capabilities: RuntimeCapabilities,
) -> Result<Value, VmError> {
    invoke_for_runtime(name, args, capabilities, 0)
}

pub fn invoke_for_runtime(
    name: &str,
    args: Vec<Value>,
    capabilities: RuntimeCapabilities,
    runtime_id: u64,
) -> Result<Value, VmError> {
    let signature =
        stdlib::native::lookup(name).ok_or_else(|| failure(name, "function is not registered"))?;
    if args.len() != signature.params.len() {
        return Err(failure(
            name,
            &format!(
                "expected {} arguments, found {}",
                signature.params.len(),
                args.len()
            ),
        ));
    }
    require_capability(name, signature.capability, capabilities)?;
    stdlib::native::with_runtime_context(runtime_id, || dispatch(name, args, runtime_id))
        .map_err(|message| failure(name, &message))
}

fn dispatch(name: &str, mut args: Vec<Value>, runtime_id: u64) -> Result<Value, String> {
    macro_rules! take {
        () => {
            args.remove(0)
        };
    }
    macro_rules! string {
        () => {
            expect_string(take!())?
        };
    }
    macro_rules! bytes {
        () => {
            expect_bytes(take!())?
        };
    }
    macro_rules! int {
        () => {
            expect_int(take!())?
        };
    }
    macro_rules! float {
        () => {
            expect_float(take!())?
        };
    }
    macro_rules! array {
        () => {
            expect_array(take!())?
        };
    }
    macro_rules! boolean {
        () => {
            expect_bool(take!())?
        };
    }
    Ok(match name {
        "std::text::length" => Value::Int(to_i64(stdlib::text::length(&string!()))?),
        "std::text::reverse" => Value::Str(stdlib::text::reverse(&string!())),
        "std::text::uppercase" => Value::Str(string!().to_uppercase()),
        "std::text::lowercase" => Value::Str(string!().to_lowercase()),
        "std::text::trim" => Value::Str(string!().trim().into()),
        "std::text::capitalize" => Value::Str(stdlib::text::capitalize(&string!())),
        "std::text::escape_html" => Value::Str(stdlib::text::escape_html(&string!())),
        "std::text::slugify" => Value::Str(stdlib::text::slugify(&string!())),
        "std::text::levenshtein" => {
            let a = string!();
            let b = string!();
            Value::Int(to_i64(stdlib::text::levenshtein(&a, &b))?)
        }
        "std::text::equals" => Value::Bool(string!() == string!()),
        "std::text::hash64" => Value::Int(i64::from_ne_bytes(
            stdlib::checksum::fnv1a_64(string!().as_bytes()).to_ne_bytes(),
        )),
        "std::text::contains" => {
            let text = string!();
            Value::Bool(text.contains(&string!()))
        }
        "std::text::starts_with" => {
            let text = string!();
            Value::Bool(text.starts_with(&string!()))
        }
        "std::text::ends_with" => {
            let text = string!();
            Value::Bool(text.ends_with(&string!()))
        }
        "std::text::replace" => {
            let text = string!();
            let from = string!();
            Value::Str(text.replace(&from, &string!()))
        }
        "std::text::truncate" => {
            let text = string!();
            let max = nonnegative(int!())?;
            Value::Str(stdlib::text::truncate(&text, max, &string!()))
        }
        // Phase 32: parseo desde string y substring por chars.
        // parse_int / parse_float retornan Option::None si falla el parseo,
        // Option::Some(n) si tiene exito — asi el usuario decide con match.
        "std::text::parse_int" => match stdlib::text::parse_int(&string!()) {
            Some(n) => Value::Enum {
                name: "Option".into(),
                variant: "Some".into(),
                payload: Some(Box::new(Value::Int(n))),
            },
            None => Value::Enum {
                name: "Option".into(),
                variant: "None".into(),
                payload: None,
            },
        },
        "std::text::parse_float" => match stdlib::text::parse_float(&string!()) {
            Some(f) => Value::Enum {
                name: "Option".into(),
                variant: "Some".into(),
                payload: Some(Box::new(Value::Float(f))),
            },
            None => Value::Enum {
                name: "Option".into(),
                variant: "None".into(),
                payload: None,
            },
        },
        "std::text::substring" => {
            let text = string!();
            let start = nonnegative(int!())?;
            let end = nonnegative(int!())?;
            Value::Str(stdlib::text::substring(&text, start, end))
        }
        "std::text::words" => Value::Array(
            string!()
                .split_whitespace()
                .map(|v| Value::Str(v.into()))
                .collect(),
        ),
        "std::text::lines" => {
            Value::Array(string!().lines().map(|v| Value::Str(v.into())).collect())
        }

        "std::encoding::hex_encode" => Value::Str(stdlib::encoding::hex_encode(&bytes!())),
        "std::encoding::hex_decode" => {
            Value::Bytes(stdlib::encoding::hex_decode(&string!()).map_err(error)?)
        }
        "std::encoding::base64_encode" => Value::Str(stdlib::encoding::base64_encode(&bytes!())),
        "std::encoding::base64_decode" => {
            Value::Bytes(stdlib::encoding::base64_decode(&string!()).map_err(error)?)
        }
        "std::encoding::percent_encode" => Value::Str(stdlib::encoding::percent_encode(&string!())),
        "std::encoding::percent_decode" => {
            Value::Str(stdlib::encoding::percent_decode(&string!()).map_err(error)?)
        }
        "std::encoding::utf8_encode" => Value::Bytes(string!().into_bytes()),
        "std::encoding::utf8_decode" => Value::Str(String::from_utf8(bytes!()).map_err(error)?),

        "std::checksum::fnv1a64" => Value::Int(stdlib::checksum::fnv1a_64(&bytes!()) as i64),
        "std::checksum::crc32" => Value::Int(i64::from(stdlib::checksum::crc32(&bytes!()))),
        "std::checksum::constant_time_eq" => {
            let a = bytes!();
            Value::Bool(stdlib::checksum::constant_time_eq(&a, &bytes!()))
        }
        "std::bytes::from_array" => Value::Bytes(
            array!()
                .into_iter()
                .map(|value| {
                    let value = expect_int(value)?;
                    u8::try_from(value).map_err(|_| "byte values must be between 0 and 255".into())
                })
                .collect::<Result<Vec<_>, String>>()?,
        ),
        "std::bytes::to_array" => Value::Array(
            bytes!()
                .into_iter()
                .map(|value| Value::Int(i64::from(value)))
                .collect(),
        ),
        "std::bytes::length" => Value::Int(to_i64(bytes!().len())?),
        "std::bytes::concat" => {
            let mut left = bytes!();
            left.extend(bytes!());
            Value::Bytes(left)
        }
        "std::bytes::slice" => {
            let values = bytes!();
            let start = nonnegative(int!())?;
            let end = nonnegative(int!())?;
            if start > end || end > values.len() {
                return Err("invalid byte slice bounds".into());
            }
            Value::Bytes(values[start..end].to_vec())
        }
        "std::bytes::read_u32_le" => {
            let values = bytes!();
            let offset = nonnegative(int!())?;
            let end = offset.checked_add(4).ok_or("byte offset overflow")?;
            let data: [u8; 4] = values
                .get(offset..end)
                .ok_or("not enough bytes for u32")?
                .try_into()
                .map_err(|_| "not enough bytes for u32")?;
            Value::Int(i64::from(u32::from_le_bytes(data)))
        }
        "std::bytes::write_u32_le" => {
            let value = u32::try_from(int!()).map_err(|_| "u32 value out of range")?;
            Value::Bytes(value.to_le_bytes().to_vec())
        }

        "std::http::parse_request" => {
            let parsed =
                stdlib::http::parse_request(&bytes!(), &stdlib::http::HttpLimits::default())
                    .map_err(error)?;
            if let Some((request, consumed)) = parsed {
                let mut map = BTreeMap::new();
                map.insert("method".into(), Value::Str(request.method));
                map.insert("target".into(), Value::Str(request.target));
                map.insert("path".into(), Value::Str(request.path));
                map.insert(
                    "query".into(),
                    request.query.map(Value::Str).unwrap_or(Value::Nil),
                );
                map.insert("version".into(), Value::Str(request.version));
                map.insert(
                    "headers".into(),
                    Value::Map(
                        request
                            .headers
                            .into_iter()
                            .map(|(key, value)| (key, Value::Str(value)))
                            .collect(),
                    ),
                );
                map.insert("body".into(), Value::Bytes(request.body));
                map.insert("keep_alive".into(), Value::Bool(request.keep_alive));
                map.insert("consumed".into(), Value::Int(consumed as i64));
                Value::Enum {
                    name: "Option".into(),
                    variant: "Some".into(),
                    payload: Some(Box::new(Value::Map(map))),
                }
            } else {
                Value::Enum {
                    name: "Option".into(),
                    variant: "None".into(),
                    payload: None,
                }
            }
        }
        "std::http::build_response" => {
            let status = u16::try_from(int!()).map_err(|_| "HTTP status out of range")?;
            let headers = expect_map(take!())?
                .into_iter()
                .map(|(key, value)| Ok((key, expect_string(value)?)))
                .collect::<Result<BTreeMap<_, _>, String>>()?;
            let body = bytes!();
            let keep_alive = expect_bool(take!())?;
            Value::Bytes(
                stdlib::http::build_response(status, &headers, &body, keep_alive).map_err(error)?,
            )
        }
        "std::http::reason_phrase" => {
            let status = u16::try_from(int!()).map_err(|_| "HTTP status out of range")?;
            stdlib::http::reason_phrase(status)
                .map(|value| Value::Str(value.into()))
                .unwrap_or(Value::Nil)
        }
        "std::http::route_match" => {
            let pattern = string!();
            let path = string!();
            match stdlib::http::match_route(&pattern, &path).map_err(error)? {
                Some(params) => Value::Enum {
                    name: "Option".into(),
                    variant: "Some".into(),
                    payload: Some(Box::new(Value::Map(
                        params
                            .into_iter()
                            .map(|(key, value)| (key, Value::Str(value)))
                            .collect(),
                    ))),
                },
                None => Value::Enum {
                    name: "Option".into(),
                    variant: "None".into(),
                    payload: None,
                },
            }
        }
        "std::http::parse_query" => {
            let query = string!();
            let limit = nonnegative(int!())?;
            Value::Map(
                stdlib::http::parse_query(&query, limit)
                    .map_err(error)?
                    .into_iter()
                    .map(|(key, values)| {
                        (
                            key,
                            Value::Array(values.into_iter().map(Value::Str).collect()),
                        )
                    })
                    .collect(),
            )
        }
        "std::http::security_headers" => with_response_headers(take!(), |headers| {
            headers
                .entry("X-Content-Type-Options".into())
                .or_insert_with(|| Value::Str("nosniff".into()));
            headers
                .entry("X-Frame-Options".into())
                .or_insert_with(|| Value::Str("DENY".into()));
            headers
                .entry("Referrer-Policy".into())
                .or_insert_with(|| Value::Str("no-referrer".into()));
            headers
                .entry("Content-Security-Policy".into())
                .or_insert_with(|| Value::Str("default-src 'none'; frame-ancestors 'none'".into()));
            Ok(())
        })?,
        "std::http::cors" => {
            let response = take!();
            let origin = string!();
            let methods = string!();
            if origin.contains(['\r', '\n']) || methods.contains(['\r', '\n']) {
                return Err("invalid CORS header value".into());
            }
            with_response_headers(response, |headers| {
                headers.insert("Access-Control-Allow-Origin".into(), Value::Str(origin));
                headers.insert("Access-Control-Allow-Methods".into(), Value::Str(methods));
                headers.insert("Vary".into(), Value::Str("Origin".into()));
                Ok(())
            })?
        }
        "std::http::request_id" => {
            let mut request = expect_map(take!())?;
            let id = REQUEST_IDS.fetch_add(1, Ordering::Relaxed);
            request.insert("request_id".into(), Value::Str(format!("titan-{id:016x}")));
            Value::Map(request)
        }
        "std::http::rate_limit" => {
            let key = string!();
            let maximum =
                u64::try_from(int!()).map_err(|_| "rate limit maximum must be nonnegative")?;
            let window =
                u64::try_from(int!()).map_err(|_| "rate limit window must be nonnegative")?;
            Value::Bool(rate_limit(
                runtime_id,
                &key,
                maximum,
                Duration::from_millis(window),
            )?)
        }
        "std::http::json_response" => {
            let status = int!();
            let value = to_json(take!())?;
            http_response_map(
                status,
                "application/json; charset=utf-8",
                serde_json::to_vec(&value).map_err(error)?,
            )
        }
        "std::http::error_response" => {
            let status = int!();
            let message = string!();
            let body = serde_json::to_vec(&serde_json::json!({"error":message})).map_err(error)?;
            http_response_map(status, "application/json; charset=utf-8", body)
        }
        "std::http::request" => {
            let method = string!();
            let url = string!();
            let headers = expect_map(take!())?
                .into_iter()
                .map(|(key, value)| Ok((key, expect_string(value)?)))
                .collect::<Result<BTreeMap<_, _>, String>>()?;
            let body = bytes!();
            let maximum = nonnegative(int!())?;
            let redirects = nonnegative(int!())?;
            let timeout = u64::try_from(int!()).map_err(|_| "HTTP timeout must be nonnegative")?;
            let response = stdlib::http_client::request(stdlib::http_client::Request {
                method,
                url,
                headers,
                body,
                maximum_body: maximum,
                redirects,
                timeout: Duration::from_millis(timeout),
            })
            .map_err(error)?;
            Value::Map(BTreeMap::from([
                ("status".into(), Value::Int(response.status as i64)),
                (
                    "headers".into(),
                    Value::Map(
                        response
                            .headers
                            .into_iter()
                            .map(|(key, value)| (key, Value::Str(value)))
                            .collect(),
                    ),
                ),
                ("body".into(), Value::Bytes(response.body)),
                ("final_url".into(), Value::Str(response.final_url)),
            ]))
        }
        "std::http::parse_multipart" => {
            let content_type = string!();
            let body = bytes!();
            let max_parts = nonnegative(int!())?;
            let max_part = nonnegative(int!())?;
            Value::Array(
                stdlib::multipart::parse(&content_type, &body, max_parts, max_part)
                    .map_err(error)?
                    .into_iter()
                    .map(|part| {
                        Value::Map(BTreeMap::from([
                            ("name".into(), Value::Str(part.name)),
                            (
                                "filename".into(),
                                part.filename.map(Value::Str).unwrap_or(Value::Nil),
                            ),
                            (
                                "content_type".into(),
                                part.content_type.map(Value::Str).unwrap_or(Value::Nil),
                            ),
                            (
                                "headers".into(),
                                Value::Map(
                                    part.headers
                                        .into_iter()
                                        .map(|(key, value)| (key, Value::Str(value)))
                                        .collect(),
                                ),
                            ),
                            ("data".into(), Value::Bytes(part.data)),
                        ]))
                    })
                    .collect(),
            )
        }
        "std::metrics::counter_add" => {
            let name = string!();
            let amount = u64::try_from(int!()).map_err(|_| "counter amount must be nonnegative")?;
            Value::Int(
                i64::try_from(stdlib::metrics::counter_add(&name, amount).map_err(error)?)
                    .unwrap_or(i64::MAX),
            )
        }
        "std::metrics::counter_get" => Value::Int(
            i64::try_from(stdlib::metrics::counter_get(&string!()).map_err(error)?)
                .unwrap_or(i64::MAX),
        ),
        "std::metrics::gauge_set" => {
            let name = string!();
            let value = float!();
            stdlib::metrics::gauge_set(&name, value).map_err(error)?;
            Value::Nil
        }
        "std::metrics::gauge_get" => {
            Value::Float(stdlib::metrics::gauge_get(&string!()).map_err(error)?)
        }
        "std::metrics::histogram_record" => {
            let name = string!();
            let value = float!();
            stdlib::metrics::histogram_record(&name, value).map_err(error)?;
            Value::Nil
        }
        "std::metrics::snapshot" => metrics_snapshot(stdlib::metrics::snapshot().map_err(error)?),
        "std::metrics::prometheus_export" => {
            Value::Str(stdlib::metrics::prometheus_export().map_err(error)?)
        }
        "std::metrics::reset" => {
            stdlib::metrics::reset().map_err(error)?;
            Value::Nil
        }
        "std::ws::accept_key" => {
            Value::Str(stdlib::websocket::accept_key(&string!()).map_err(error)?)
        }
        "std::ws::upgrade_response" => {
            let key = string!();
            let protocol = string!();
            Value::Bytes(
                stdlib::websocket::upgrade_response(
                    &key,
                    if protocol.is_empty() {
                        None
                    } else {
                        Some(&protocol)
                    },
                )
                .map_err(error)?,
            )
        }
        "std::ws::validate_upgrade" => {
            let request = take!();
            let protocol = string!();
            Value::Bytes(websocket_upgrade(request, &protocol)?)
        }
        "std::ws::validate_accept" => {
            let response = bytes!();
            let key = string!();
            Value::Bool(websocket_validate_accept(&response, &key)?)
        }
        "std::ws::encode" => {
            let opcode = u8::try_from(int!()).map_err(|_| "WebSocket opcode out of range")?;
            let payload = bytes!();
            let masked = expect_bool(take!())?;
            Value::Bytes(
                stdlib::websocket::encode_frame_with_policy(opcode, &payload, masked)
                    .map_err(error)?,
            )
        }
        "std::ws::parse" => {
            let data = bytes!();
            let require_mask = expect_bool(take!())?;
            let maximum = nonnegative(int!())?;
            match stdlib::websocket::parse_frame(&data, Some(require_mask), maximum)
                .map_err(error)?
            {
                Some(frame) => Value::Enum {
                    name: "Option".into(),
                    variant: "Some".into(),
                    payload: Some(Box::new(Value::Map(BTreeMap::from([
                        ("fin".into(), Value::Bool(frame.fin)),
                        ("opcode".into(), Value::Int(frame.opcode as i64)),
                        ("payload".into(), Value::Bytes(frame.payload)),
                        ("consumed".into(), Value::Int(frame.consumed as i64)),
                    ])))),
                },
                None => Value::Enum {
                    name: "Option".into(),
                    variant: "None".into(),
                    payload: None,
                },
            }
        }
        "std::csv::parse" => Value::Array(
            stdlib::csv::parse(&string!())
                .map_err(error)?
                .into_iter()
                .map(|row| Value::Array(row.into_iter().map(Value::Str).collect()))
                .collect(),
        ),
        "std::csv::serialize" => {
            let rows = array!()
                .into_iter()
                .map(expect_string_array)
                .collect::<Result<Vec<_>, _>>()?;
            Value::Str(stdlib::csv::serialize(&rows))
        }
        "std::json::parse" => from_json(stdlib::json::parse(&string!()).map_err(error)?)?,
        "std::json::stringify" => Value::Str(stdlib::json::stringify(&to_json(take!())?)),
        "std::json::pretty" => {
            Value::Str(stdlib::json::stringify_pretty(&to_json(take!())?).map_err(error)?)
        }
        "std::json::pointer" => {
            let value = to_json(take!())?;
            let pointer = string!();
            value
                .pointer(&pointer)
                .cloned()
                .map(from_json)
                .transpose()?
                .unwrap_or(Value::Nil)
        }
        "std::json::merge" => {
            let mut target = to_json(take!())?;
            let patch = to_json(take!())?;
            stdlib::json::merge(&mut target, patch);
            from_json(target)?
        }
        "std::json::flatten" => Value::Array(
            stdlib::json::flatten(&to_json(take!())?)
                .into_iter()
                .map(|(path, value)| Ok(Value::Tuple(vec![Value::Str(path), from_json(value)?])))
                .collect::<Result<Vec<_>, String>>()?,
        ),

        "std::array::set" => {
            let mut values = array!();
            let index = nonnegative(int!())?;
            let value = take!();
            let Some(slot) = values.get_mut(index) else {
                return Err("array index out of bounds".into());
            };
            *slot = value;
            Value::Array(values)
        }
        "std::array::push" => {
            let mut values = array!();
            values.push(take!());
            Value::Array(values)
        }
        "std::array::pop" => {
            let mut values = array!();
            let _ = values.pop();
            Value::Array(values)
        }
        "std::array::slice" => {
            let values = array!();
            let start = nonnegative(int!())?;
            let end = nonnegative(int!())?;
            if start > end || end > values.len() {
                return Err("invalid array slice range".into());
            }
            Value::Array(values[start..end].to_vec())
        }
        "std::array::concat" => {
            let mut left = array!();
            left.extend(array!());
            Value::Array(left)
        }
        "std::array::filled" => {
            let n = nonnegative(int!())?;
            let value = take!();
            Value::Array((0..n).map(|_| value.clone()).collect())
        }
        "std::array::range" => {
            let start = int!();
            let end = int!();
            if end < start {
                return Err("range end must be >= start".into());
            }
            Value::Array((start..end).map(Value::Int).collect())
        }
        "std::collections::length" => Value::Int(to_i64(value_length(&take!())?)?),
        "std::collections::contains" => {
            let values = array!();
            Value::Bool(values.contains(&take!()))
        }
        "std::collections::reverse" => {
            let mut values = array!();
            values.reverse();
            Value::Array(values)
        }
        "std::collections::deduplicate" => {
            let mut output = Vec::new();
            for value in array!() {
                if !output.contains(&value) {
                    output.push(value);
                }
            }
            Value::Array(output)
        }
        "std::collections::join" => {
            let values = array!();
            let separator = string!();
            Value::Str(
                values
                    .iter()
                    .map(val_to_string)
                    .collect::<Vec<_>>()
                    .join(&separator),
            )
        }
        "std::collections::chunk" => {
            let values = array!();
            let size = nonnegative(int!())?;
            if size == 0 {
                return Err("chunk size must be positive".into());
            }
            Value::Array(
                values
                    .chunks(size)
                    .map(|part| Value::Array(part.to_vec()))
                    .collect(),
            )
        }
        "std::map::new" => Value::Map(BTreeMap::new()),
        "std::map::length" => Value::Int(to_i64(expect_map(take!())?.len())?),
        "std::map::insert_new" => {
            let mut values = expect_map(take!())?;
            let key = string!();
            if values.contains_key(&key) {
                return Err("map key already exists".into());
            }
            values.insert(key, take!());
            Value::Map(values)
        }
        "std::map::keys" => {
            Value::Array(expect_map(take!())?.into_keys().map(Value::Str).collect())
        }
        "std::map::values" => Value::Array(expect_map(take!())?.into_values().collect()),
        "std::map::contains" => {
            let values = expect_map(take!())?;
            Value::Bool(values.contains_key(&string!()))
        }
        "std::map::get" => {
            let values = expect_map(take!())?;
            values.get(&string!()).cloned().unwrap_or(Value::Nil)
        }
        "std::map::insert" => {
            let mut values = expect_map(take!())?;
            let key = string!();
            values.insert(key, take!());
            Value::Map(values)
        }
        "std::map::remove" => {
            let mut values = expect_map(take!())?;
            values.remove(&string!());
            Value::Map(values)
        }

        "std::math::sqrt" => checked_float(float!().sqrt(), "sqrt domain error")?,
        "std::math::pow" => {
            let base = float!();
            checked_float(base.powf(float!()), "pow domain error")?
        }
        "std::math::sin" => Value::Float(float!().sin()),
        "std::math::cos" => Value::Float(float!().cos()),
        "std::math::tan" => checked_float(float!().tan(), "tan produced a non-finite result")?,
        "std::math::ln" => checked_float(float!().ln(), "ln domain error")?,
        "std::math::abs" => Value::Float(float!().abs()),
        "std::math::floor" => Value::Float(float!().floor()),
        "std::math::ceil" => Value::Float(float!().ceil()),
        "std::math::round" => Value::Float(float!().round()),
        "std::math::exp" => Value::Float(float!().exp()),
        "std::math::log" => {
            let x = float!();
            let base = float!();
            Value::Float(x.log(base))
        }
        "std::math::to_float" => Value::Float(int!() as f64),
        "std::math::to_int" => Value::Int(float!() as i64),
        "std::stats::mean" => {
            let values = numbers(array!())?;
            if values.is_empty() {
                return Err("mean requires at least one number".into());
            }
            Value::Float(values.iter().sum::<f64>() / values.len() as f64)
        }
        "std::stats::median" => {
            let mut values = numbers(array!())?;
            Value::Float(
                stdlib::stats::median(&mut values).ok_or("median requires finite numbers")?,
            )
        }
        "std::stats::quantile" => {
            let mut values = numbers(array!())?;
            let q = float!();
            Value::Float(
                stdlib::stats::quantile(&mut values, q).ok_or("invalid quantile or input")?,
            )
        }
        "std::stats::variance" => {
            let values = numbers(array!())?;
            let mut summary = stdlib::stats::Summary::new();
            summary.extend(values);
            Value::Float(
                summary
                    .variance_population()
                    .ok_or("variance requires numbers")?,
            )
        }
        "std::stats::stddev" => {
            let values = numbers(array!())?;
            let mut summary = stdlib::stats::Summary::new();
            summary.extend(values);
            Value::Float(
                summary
                    .standard_deviation()
                    .ok_or("standard deviation requires numbers")?,
            )
        }

        "std::time::unix_seconds" => {
            Value::Int(i64::try_from(stdlib::time::unix_seconds().map_err(error)?).map_err(error)?)
        }
        "std::time::unix_millis" => {
            Value::Int(i64::try_from(stdlib::time::unix_millis().map_err(error)?).map_err(error)?)
        }
        "std::time::sleep_ms" => {
            stdlib::time::sleep(Duration::from_millis(
                u64::try_from(int!()).map_err(|_| "milliseconds must be nonnegative")?,
            ));
            Value::Nil
        }

        "std::path::join" => Value::Str(
            stdlib::path::join(string!(), string!())
                .to_string_lossy()
                .into(),
        ),
        "std::path::normalize" => {
            Value::Str(stdlib::path::normalize(string!()).to_string_lossy().into())
        }
        "std::path::parent" => optional_path(stdlib::path::parent(string!())),
        "std::path::file_name" => optional_string(stdlib::path::file_name(string!())),
        "std::path::stem" => optional_string(stdlib::path::stem(string!())),
        "std::path::extension" => optional_string(stdlib::path::extension(string!())),
        "std::path::absolute" => Value::Str(
            stdlib::path::absolute(string!())
                .map_err(error)?
                .to_string_lossy()
                .into(),
        ),
        "std::path::canonical" => Value::Str(
            stdlib::path::canonical(string!())
                .map_err(error)?
                .to_string_lossy()
                .into(),
        ),

        "std::fs::read_text" => Value::Str(stdlib::io::read_file(string!()).map_err(error)?),
        "std::fs::read_bytes" => Value::Bytes(stdlib::io::read_bytes(string!()).map_err(error)?),
        "std::fs::write_text" => {
            let path = string!();
            stdlib::io::write_file(path, &string!()).map_err(error)?;
            Value::Nil
        }
        "std::fs::write_bytes" => {
            let path = string!();
            stdlib::io::write_bytes(path, &bytes!()).map_err(error)?;
            Value::Nil
        }
        "std::fs::atomic_write" => {
            let path = string!();
            stdlib::io::atomic_write(path, &bytes!()).map_err(error)?;
            Value::Nil
        }
        "std::fs::append" => {
            let path = string!();
            stdlib::io::append(path, &bytes!()).map_err(error)?;
            Value::Nil
        }
        "std::fs::exists" => Value::Bool(stdlib::io::exists(string!())),
        "std::fs::is_file" => Value::Bool(stdlib::io::is_file(string!())),
        "std::fs::is_dir" => Value::Bool(stdlib::io::is_dir(string!())),
        "std::fs::create_dir" => {
            stdlib::io::create_dir(string!()).map_err(error)?;
            Value::Nil
        }
        "std::fs::remove_file" => {
            stdlib::io::remove_file(string!()).map_err(error)?;
            Value::Nil
        }
        "std::fs::remove_dir" => {
            stdlib::io::remove_dir(string!()).map_err(error)?;
            Value::Nil
        }
        "std::fs::list_dir" => Value::Array(
            stdlib::io::list_dir(string!())
                .map_err(error)?
                .into_iter()
                .map(|p| Value::Str(p.to_string_lossy().into()))
                .collect(),
        ),
        "std::fs::file_size" => Value::Int(
            i64::try_from(stdlib::io::file_size(string!()).map_err(error)?).map_err(error)?,
        ),
        "std::fs::copy" => {
            let from = string!();
            Value::Int(
                i64::try_from(stdlib::io::copy(from, string!()).map_err(error)?).map_err(error)?,
            )
        }
        "std::fs::rename" => {
            let from = string!();
            stdlib::io::rename(from, string!()).map_err(error)?;
            Value::Nil
        }

        // Legacy run(program, args): superseded by Phase 34's process_mod::run(command).
        // When process_mod is on this arm would be an unreachable duplicate match arm.
        #[cfg(not(feature = "process_mod"))]
        "std::process::run" => {
            let program = string!();
            process_output(
                stdlib::process::CommandSpec::new(program)
                    .args(strings(array!())?)
                    .output()
                    .map_err(error)?,
            )
        }
        "std::process::run_timeout" => {
            let program = string!();
            let arguments = strings(array!())?;
            let timeout = u64::try_from(int!()).map_err(|_| "timeout must be nonnegative")?;
            process_output(
                stdlib::process::CommandSpec::new(program)
                    .args(arguments)
                    .output_timeout(Duration::from_millis(timeout))
                    .map_err(error)?,
            )
        }
        "std::env::get" => std::env::var(string!())
            .map(Value::Str)
            .unwrap_or(Value::Nil),
        "std::env::args" => Value::Array(std::env::args().map(Value::Str).collect()),
        "std::env::current_dir" => Value::Str(
            std::env::current_dir()
                .map_err(error)?
                .to_string_lossy()
                .into(),
        ),
        "std::net::http_get" => {
            let response = stdlib::net::http_get(&string!()).map_err(error)?;
            let mut map = BTreeMap::new();
            map.insert("status".into(), Value::Int(i64::from(response.status)));
            map.insert("body".into(), Value::Bytes(response.body));
            map.insert(
                "headers".into(),
                Value::Array(
                    response
                        .headers
                        .into_iter()
                        .map(|(k, v)| Value::Tuple(vec![Value::Str(k), Value::Str(v)]))
                        .collect(),
                ),
            );
            Value::Map(map)
        }
        "std::web::query_exists"
        | "std::web::set_text"
        | "std::web::set_html"
        | "std::web::set_attribute"
        | "std::web::add_class"
        | "std::web::remove_class"
        | "std::web::focus"
        | "std::web::set_title"
        | "std::web::listen"
        | "std::web::unlisten"
        | "std::web::event_type"
        | "std::web::event_value"
        | "std::web::event_key"
        | "std::web::event_target_id"
        | "std::web::event_checked"
        | "std::web::event_x"
        | "std::web::event_y"
        | "std::web::fetch"
        | "std::web::fetch_cancel"
        | "std::web::fetch_ok"
        | "std::web::fetch_status"
        | "std::web::fetch_body"
        | "std::web::fetch_url"
        | "std::web::fetch_error"
        | "std::web::fetch_headers"
        | "std::web::request"
        | "std::web::ws_connect"
        | "std::web::ws_send"
        | "std::web::ws_close"
        | "std::web::ws_id"
        | "std::web::ws_message"
        | "std::web::ws_protocol"
        | "std::web::ws_close_code"
        | "std::web::ws_close_reason"
        | "std::web::ws_was_clean"
        | "std::web::ws_error"
        | "std::web::canvas_resize"
        | "std::web::canvas_clear"
        | "std::web::canvas_fill_rect"
        | "std::web::canvas_stroke_rect"
        | "std::web::canvas_line"
        | "std::web::canvas_text"
        | "std::web::animation_start"
        | "std::web::animation_cancel"
        | "std::web::frame_id"
        | "std::web::frame_time_ms"
        | "std::web::frame_delta_ms"
        | "std::web::frame_count"
        | "std::web::webgl_supported"
        | "std::web::webgl_create"
        | "std::web::webgl_uniform_f32"
        | "std::web::webgl_draw"
        | "std::web::webgl_delete" => {
            return Err("std::web functions require the WebAssembly browser host".into())
        }
        "std::wasm::heap_used"
        | "std::wasm::heap_capacity"
        | "std::wasm::heap_limit"
        | "std::wasm::heap_set_limit"
        | "std::wasm::heap_checkpoint"
        | "std::wasm::heap_restore"
        | "std::wasm::heap_allocations"
        | "std::wasm::heap_allocated_bytes"
        | "std::wasm::heap_restores"
        | "std::wasm::heap_reclaimed_bytes"
        | "std::wasm::heap_peak_used"
        | "std::wasm::heap_reset_counters"
        | "std::wasm::heap_scope_begin"
        | "std::wasm::heap_scope_end" => {
            return Err("std::wasm heap functions require the WebAssembly backend".into())
        }
        "std::window::create" => {
            let title = string!();
            let width = u32::try_from(int!()).map_err(|_| "window width out of range")?;
            let height = u32::try_from(int!()).map_err(|_| "window height out of range")?;
            let id = stdlib::window::create(&title, width, height).map_err(error)?;
            Value::Int(i64::try_from(id).map_err(|_| "window handle out of range")?)
        }
        "std::window::is_open" => {
            let id = u64::try_from(int!()).map_err(|_| "window handle must be nonnegative")?;
            Value::Bool(stdlib::window::is_open(id))
        }
        "std::window::close" => {
            let id = u64::try_from(int!()).map_err(|_| "window handle must be nonnegative")?;
            Value::Bool(stdlib::window::close(id))
        }
        "std::window::set_title" => {
            let id = u64::try_from(int!()).map_err(|_| "window handle must be nonnegative")?;
            let title = string!();
            Value::Bool(stdlib::window::set_title(id, &title))
        }
        "std::window::resize" => {
            let id = u64::try_from(int!()).map_err(|_| "window handle must be nonnegative")?;
            let width = u32::try_from(int!()).map_err(|_| "window width out of range")?;
            let height = u32::try_from(int!()).map_err(|_| "window height out of range")?;
            Value::Bool(stdlib::window::resize(id, width, height))
        }
        "std::window::poll_events" => {
            let id = u64::try_from(int!()).map_err(|_| "window handle must be nonnegative")?;
            Value::Array(
                stdlib::window::poll_events(id)
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(all(feature = "window_live", not(target_os = "android")))]
        "std::window::live_open" => {
            let title = string!();
            let width = u32::try_from(int!()).map_err(|_| "window width out of range")?;
            let height = u32::try_from(int!()).map_err(|_| "window height out of range")?;
            Value::Int(stdlib::window_live::live_open(&title, width, height))
        }
        #[cfg(all(feature = "window_live", not(target_os = "android")))]
        "std::window::live_is_open" => {
            let id = int!();
            Value::Bool(stdlib::window_live::live_is_open(id))
        }
        #[cfg(all(feature = "window_live", not(target_os = "android")))]
        "std::window::live_close" => {
            let id = int!();
            Value::Bool(stdlib::window_live::live_close(id))
        }
        #[cfg(all(feature = "window_live", not(target_os = "android")))]
        "std::window::live_set_title" => {
            let id = int!();
            let title = string!();
            Value::Bool(stdlib::window_live::live_set_title(id, &title))
        }
        #[cfg(all(feature = "window_live", not(target_os = "android")))]
        "std::window::live_pump" => {
            let id = int!();
            let gui = int!();
            Value::Int(stdlib::window_live::live_pump(id, gui))
        }
        #[cfg(all(feature = "window_live", not(target_os = "android")))]
        "std::window::live_poll_events" => {
            let id = int!();
            Value::Array(
                stdlib::window_live::live_poll_events(id)
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }

        "std::input::is_key_pressed" => {
            let key = string!();
            Value::Bool(stdlib::input::is_key_pressed(&key))
        }
        "std::input::mouse_pos" => {
            let (x, y) = stdlib::input::mouse_pos();
            Value::Array(vec![Value::Int(i64::from(x)), Value::Int(i64::from(y))])
        }
        "std::input::is_mouse_button_pressed" => {
            let btn = int!() as u8;
            Value::Bool(stdlib::input::is_mouse_button_pressed(btn))
        }
        "std::input::touch_pos" => {
            let idx = int!() as u32;
            let (x, y, active) = stdlib::input::touch_pos(idx);
            Value::Array(vec![
                Value::Int(i64::from(x)),
                Value::Int(i64::from(y)),
                Value::Bool(active),
            ])
        }
        "std::input::set_key_state" => {
            let key = string!();
            let pressed = boolean!();
            Value::Bool(stdlib::input::set_key_state(&key, pressed))
        }
        "std::input::set_mouse_pos" => {
            let x = int!() as i32;
            let y = int!() as i32;
            Value::Bool(stdlib::input::set_mouse_pos(x, y))
        }
        "std::input::set_mouse_button" => {
            let btn = int!() as u8;
            let pressed = boolean!();
            Value::Bool(stdlib::input::set_mouse_button(btn, pressed))
        }
        "std::input::set_touch_point" => {
            let idx = int!() as u32;
            let x = int!() as i32;
            let y = int!() as i32;
            let active = boolean!();
            Value::Bool(stdlib::input::set_touch_point(idx, x, y, active))
        }
        "std::clipboard::get_text" => Value::Str(stdlib::clipboard::get_text()),
        "std::clipboard::set_text" => {
            let text = string!();
            Value::Bool(stdlib::clipboard::set_text(&text))
        }
        "std::notify::send" => {
            let title = string!();
            let body = string!();
            Value::Bool(stdlib::clipboard::send_notification(&title, &body))
        }
        "std::mobile::state" => Value::Str(stdlib::mobile::get_state()),
        "std::mobile::trigger" => {
            let event = string!();
            Value::Bool(stdlib::mobile::trigger_event(&event))
        }
        "std::mobile::poll_events" => Value::Array(
            stdlib::mobile::poll_events()
                .into_iter()
                .map(Value::Str)
                .collect(),
        ),
        // Phase 8: Game & Audio
        "std::game::init" => {
            let title = string!();
            let width = int!();
            let height = int!();
            Value::Bool(titan_game_init(&title, width, height))
        }
        "std::game::step" => Value::Float(titan_game_step()),
        "std::game::fps" => Value::Int(titan_game_fps()),
        "std::game::check_collision" => {
            let x1 = float!();
            let y1 = float!();
            let w1 = float!();
            let h1 = float!();
            let x2 = float!();
            let y2 = float!();
            let w2 = float!();
            let h2 = float!();
            Value::Bool(titan_game_check_collision(
                (x1, y1),
                (w1, h1),
                (x2, y2),
                (w2, h2),
            ))
        }
        "std::game::shutdown" => Value::Bool(titan_game_shutdown()),
        // Legacy in-memory audio module renamed to `sim_*` in 0.7.0 so
        // the real `std::audio::*` (Phase 9: hound + termux-media) can
        // own those names. See titan_stdlib/src/native.rs.
        "std::audio::sim_init" => Value::Bool(titan_audio_init()),
        "std::audio::sim_load_wave" => {
            let freq_hz = float!();
            let duration_ms = int!();
            Value::Int(titan_audio_load_wave(freq_hz, duration_ms))
        }
        "std::audio::sim_sample_count" => {
            let handle = int!();
            Value::Int(titan_audio_sample_count(handle) as i64)
        }
        "std::audio::sim_play" => {
            let handle = int!();
            let loop_audio = boolean!();
            Value::Bool(titan_audio_play(handle, loop_audio))
        }
        "std::audio::sim_set_volume" => {
            let handle = int!();
            let volume = float!();
            Value::Bool(titan_audio_set_volume(handle, volume))
        }
        "std::audio::sim_stop" => {
            let handle = int!();
            Value::Bool(titan_audio_stop(handle))
        }
        // Phase 8: GUI Native Bindings
        "std::gui::init" => Value::Bool(titan_gui_init()),
        "std::gui::create_container" => {
            let title = string!();
            let width = int!();
            let height = int!();
            Value::Int(titan_gui_create_container(&title, width, height))
        }
        "std::gui::add_button" => {
            let parent = int!();
            let label = string!();
            let x = int!();
            let y = int!();
            let w = int!();
            let h = int!();
            Value::Int(titan_gui_add_button(parent, &label, x, y, w, h))
        }
        "std::gui::add_label" => {
            let parent = int!();
            let text = string!();
            let x = int!();
            let y = int!();
            Value::Int(titan_gui_add_label(parent, &text, x, y))
        }
        "std::gui::set_text" => {
            let id = int!();
            let text = string!();
            Value::Bool(titan_gui_set_text(id, &text))
        }
        "std::gui::get_text" => {
            let id = int!();
            Value::Str(titan_gui_get_text(id))
        }
        "std::gui::trigger_click" => {
            let id = int!();
            Value::Bool(titan_gui_trigger_click(id))
        }
        "std::gui::is_clicked" => {
            let id = int!();
            Value::Bool(titan_gui_is_clicked(id))
        }
        "std::gui::child_count" => {
            let id = int!();
            Value::Int(titan_gui_child_count(id) as i64)
        }
        "std::gui::shutdown" => Value::Bool(titan_gui_shutdown()),
        // Fase 2: rasterizador por software — pixeles RGBA reales del arbol.
        "std::gui::render" => {
            let id = int!();
            match stdlib::gui_raster::render_rgba(id) {
                Some((_w, _h, pixels)) => Value::Bytes(pixels),
                None => Value::Nil,
            }
        }
        // Phase 9: Freestanding & Bare-Metal Bindings
        "std::freestanding::init" => {
            let arch = string!();
            Value::Bool(titan_freestanding_init(&arch))
        }
        "std::freestanding::validate_target_spec" => {
            let target = string!();
            Value::Bool(titan_freestanding_validate_target_spec(&target))
        }
        "std::freestanding::generate_linker_script" => {
            let arch = string!();
            let base = int!();
            let stack = int!();
            Value::Str(titan_freestanding_generate_linker_script(
                &arch,
                base as u64,
                stack as u64,
            ))
        }
        "std::freestanding::generate_startup_asm" => {
            let arch = string!();
            let entry = string!();
            Value::Str(titan_freestanding_generate_startup_asm(&arch, &entry))
        }
        "std::freestanding::get_active_target" => {
            Value::Str(titan_freestanding_get_active_target())
        }
        "std::freestanding::shutdown" => Value::Bool(titan_freestanding_shutdown()),
        // Phase 9: Freestanding Memory & Paging Bindings
        "std::freestanding_memory::init_frame_allocator" => {
            let base = int!();
            let size = int!();
            Value::Bool(titan_freestanding_memory_init_frame_allocator(
                base as u64,
                size as u64,
            ))
        }
        "std::freestanding_memory::allocate_frame" => {
            Value::Int(titan_freestanding_memory_allocate_frame() as i64)
        }
        "std::freestanding_memory::deallocate_frame" => {
            let paddr = int!();
            Value::Bool(titan_freestanding_memory_deallocate_frame(paddr as u64))
        }
        "std::freestanding_memory::map_page" => {
            let vaddr = int!();
            let paddr = int!();
            let flags = int!();
            Value::Bool(titan_freestanding_memory_map_page(
                vaddr as u64,
                paddr as u64,
                flags as u32,
            ))
        }
        "std::freestanding_memory::translate_page" => {
            let vaddr = int!();
            Value::Int(titan_freestanding_memory_translate_page(vaddr as u64) as i64)
        }
        "std::freestanding_memory::free_frames_count" => {
            Value::Int(titan_freestanding_memory_free_frames_count() as i64)
        }
        "std::freestanding_memory::shutdown" => Value::Bool(titan_freestanding_memory_shutdown()),
        // Phase 9: Freestanding CPU & Exception Traps Bindings
        "std::freestanding_cpu::init_exception_table" => {
            let base = int!();
            Value::Bool(titan_freestanding_cpu_init_exception_table(base as u64))
        }
        "std::freestanding_cpu::register_exception_handler" => {
            let vec_id = int!();
            let addr = int!();
            Value::Bool(titan_freestanding_cpu_register_exception_handler(
                vec_id as u32,
                addr as u64,
            ))
        }
        "std::freestanding_cpu::dispatch_exception" => {
            let vec_id = int!();
            let fault = int!();
            let code = int!();
            Value::Int(titan_freestanding_cpu_dispatch_exception(
                vec_id as u32,
                fault as u64,
                code as u64,
            ) as i64)
        }
        "std::freestanding_cpu::register_syscall_handler" => {
            let num = int!();
            let addr = int!();
            Value::Bool(titan_freestanding_cpu_register_syscall_handler(
                num as u32,
                addr as u64,
            ))
        }
        "std::freestanding_cpu::invoke_syscall" => {
            let num = int!();
            let a0 = int!();
            let a1 = int!();
            let a2 = int!();
            Value::Int(titan_freestanding_cpu_invoke_syscall(
                num as u32, a0 as u64, a1 as u64, a2 as u64,
            ) as i64)
        }
        "std::freestanding_cpu::get_last_fault_addr" => {
            Value::Int(titan_freestanding_cpu_get_last_fault_addr() as i64)
        }
        "std::freestanding_cpu::shutdown" => Value::Bool(titan_freestanding_cpu_shutdown()),
        // Phase 9: Freestanding MMIO & UART Serial Bindings
        "std::freestanding_mmio::init_mmio_region" => {
            let base = int!();
            let size = int!();
            Value::Bool(titan_freestanding_mmio_init_mmio_region(
                base as u64,
                size as u64,
            ))
        }
        "std::freestanding_mmio::read_mmio_u32" => {
            let paddr = int!();
            Value::Int(titan_freestanding_mmio_read_mmio_u32(paddr as u64) as i64)
        }
        "std::freestanding_mmio::write_mmio_u32" => {
            let paddr = int!();
            let val = int!();
            Value::Bool(titan_freestanding_mmio_write_mmio_u32(
                paddr as u64,
                val as u32,
            ))
        }
        "std::freestanding_mmio::serial_init" => {
            let base = int!();
            let baud = int!();
            Value::Bool(titan_freestanding_mmio_serial_init(
                base as u64,
                baud as u32,
            ))
        }
        "std::freestanding_mmio::serial_write_str" => {
            let text = string!();
            Value::Int(titan_freestanding_mmio_serial_write_str(&text) as i64)
        }
        "std::freestanding_mmio::serial_get_buffer" => {
            Value::Str(titan_freestanding_mmio_serial_get_buffer())
        }
        "std::freestanding_mmio::shutdown" => Value::Bool(titan_freestanding_mmio_shutdown()),

        "std::testing::assert" => {
            let condition = take!();
            let Value::Bool(condition) = condition else {
                return Err("assert condition must be bool".into());
            };
            let message = string!();
            if !condition {
                return Err(format!("assertion failed: {message}"));
            }
            Value::Nil
        }
        "std::testing::assert_eq" => {
            let left = take!();
            let right = take!();
            let message = string!();
            if left != right {
                return Err(format!(
                    "assertion failed: {message}; left={}, right={}",
                    val_to_string(&left),
                    val_to_string(&right)
                ));
            }
            Value::Nil
        }

        // ---------------- Phase 1: regex ----------------
        #[cfg(feature = "regex_mod")]
        "std::regex::is_match" => {
            let pattern = string!();
            let text = string!();
            Value::Bool(stdlib::regex_mod::is_match(&pattern, &text).map_err(error)?)
        }
        #[cfg(feature = "regex_mod")]
        "std::regex::find" => {
            let pattern = string!();
            let text = string!();
            Value::Str(stdlib::regex_mod::find(&pattern, &text).map_err(error)?)
        }
        #[cfg(feature = "regex_mod")]
        "std::regex::find_all" => {
            let pattern = string!();
            let text = string!();
            Value::Array(
                stdlib::regex_mod::find_all(&pattern, &text)
                    .map_err(error)?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "regex_mod")]
        "std::regex::captures" => {
            let pattern = string!();
            let text = string!();
            Value::Array(
                stdlib::regex_mod::captures(&pattern, &text)
                    .map_err(error)?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "regex_mod")]
        "std::regex::replace_all" => {
            let pattern = string!();
            let text = string!();
            let replacement = string!();
            Value::Str(
                stdlib::regex_mod::replace_all(&pattern, &text, &replacement).map_err(error)?,
            )
        }
        #[cfg(feature = "regex_mod")]
        "std::regex::split" => {
            let pattern = string!();
            let text = string!();
            Value::Array(
                stdlib::regex_mod::split(&pattern, &text)
                    .map_err(error)?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "regex_mod")]
        "std::regex::is_valid" => Value::Bool(stdlib::regex_mod::is_valid(&string!())),

        // ---------------- Phase 1: uuid ----------------
        #[cfg(feature = "uuid_mod")]
        "std::uuid::v4" => Value::Str(stdlib::uuid_mod::v4()),
        #[cfg(feature = "uuid_mod")]
        "std::uuid::v7" => Value::Str(stdlib::uuid_mod::v7()),
        #[cfg(feature = "uuid_mod")]
        "std::uuid::is_valid" => Value::Bool(stdlib::uuid_mod::is_valid(&string!())),
        #[cfg(feature = "uuid_mod")]
        "std::uuid::normalize" => Value::Str(stdlib::uuid_mod::normalize(&string!())),
        #[cfg(feature = "uuid_mod")]
        "std::uuid::nil" => Value::Str(stdlib::uuid_mod::nil()),

        // ---------------- Phase 1: hash ----------------
        #[cfg(feature = "hash_mod")]
        "std::hash::sha256" => Value::Str(stdlib::hash_mod::sha256(&bytes!())),
        #[cfg(feature = "hash_mod")]
        "std::hash::sha384" => Value::Str(stdlib::hash_mod::sha384(&bytes!())),
        #[cfg(feature = "hash_mod")]
        "std::hash::sha512" => Value::Str(stdlib::hash_mod::sha512(&bytes!())),
        #[cfg(feature = "hash_mod")]
        "std::hash::sha3_256" => Value::Str(stdlib::hash_mod::sha3_256(&bytes!())),
        #[cfg(feature = "hash_mod")]
        "std::hash::sha3_512" => Value::Str(stdlib::hash_mod::sha3_512(&bytes!())),
        #[cfg(feature = "hash_mod")]
        "std::hash::blake3" => Value::Str(stdlib::hash_mod::blake3(&bytes!())),
        #[cfg(feature = "hash_mod")]
        "std::hash::sha256_bytes" => Value::Bytes(stdlib::hash_mod::sha256_bytes(&bytes!())),
        #[cfg(feature = "hash_mod")]
        "std::hash::sha512_bytes" => Value::Bytes(stdlib::hash_mod::sha512_bytes(&bytes!())),
        #[cfg(feature = "hash_mod")]
        "std::hash::blake3_bytes" => Value::Bytes(stdlib::hash_mod::blake3_bytes(&bytes!())),
        #[cfg(feature = "hash_mod")]
        "std::hash::hmac_sha256" => {
            let key = bytes!();
            let data = bytes!();
            Value::Str(stdlib::hash_mod::hmac_sha256(&key, &data))
        }
        #[cfg(feature = "hash_mod")]
        "std::hash::hmac_sha512" => {
            let key = bytes!();
            let data = bytes!();
            Value::Str(stdlib::hash_mod::hmac_sha512(&key, &data))
        }

        // ---------------- Phase 1: random ----------------
        #[cfg(feature = "random_mod")]
        "std::random::int" => Value::Int(stdlib::random_mod::int()),
        #[cfg(feature = "random_mod")]
        "std::random::range" => {
            let min = int!();
            let max = int!();
            Value::Int(stdlib::random_mod::range(min, max))
        }
        #[cfg(feature = "random_mod")]
        "std::random::float" => Value::Float(stdlib::random_mod::float()),
        #[cfg(feature = "random_mod")]
        "std::random::bool" => Value::Bool(stdlib::random_mod::boolean()),
        #[cfg(feature = "random_mod")]
        "std::random::bytes" => {
            let n = nonnegative(int!())?;
            Value::Bytes(stdlib::random_mod::bytes(n))
        }
        #[cfg(feature = "random_mod")]
        "std::random::seeded_int" => {
            let seed = int!() as u64;
            let min = int!();
            let max = int!();
            Value::Int(stdlib::random_mod::seeded_int(seed, min, max))
        }
        #[cfg(feature = "random_mod")]
        "std::random::seeded_float" => {
            let seed = int!() as u64;
            Value::Float(stdlib::random_mod::seeded_float(seed))
        }
        #[cfg(feature = "random_mod")]
        "std::random::seeded_bytes" => {
            let seed = int!() as u64;
            let n = nonnegative(int!())?;
            Value::Bytes(stdlib::random_mod::seeded_bytes(seed, n))
        }

        // ---------------- Phase 1: datetime ----------------
        #[cfg(feature = "datetime_mod")]
        "std::datetime::now" => Value::Int(stdlib::datetime_mod::now()),
        #[cfg(feature = "datetime_mod")]
        "std::datetime::now_iso" => Value::Str(stdlib::datetime_mod::now_iso()),
        #[cfg(feature = "datetime_mod")]
        "std::datetime::format" => {
            let ts = int!();
            let fmt = string!();
            Value::Str(stdlib::datetime_mod::format(ts, &fmt).map_err(error)?)
        }
        #[cfg(feature = "datetime_mod")]
        "std::datetime::to_rfc3339" => {
            Value::Str(stdlib::datetime_mod::to_rfc3339(int!()).map_err(error)?)
        }
        #[cfg(feature = "datetime_mod")]
        "std::datetime::to_rfc2822" => {
            Value::Str(stdlib::datetime_mod::to_rfc2822(int!()).map_err(error)?)
        }
        #[cfg(feature = "datetime_mod")]
        "std::datetime::parse_rfc3339" => {
            Value::Int(stdlib::datetime_mod::parse_rfc3339(&string!()).map_err(error)?)
        }
        #[cfg(feature = "datetime_mod")]
        "std::datetime::parse" => {
            let text = string!();
            let fmt = string!();
            Value::Int(stdlib::datetime_mod::parse(&text, &fmt).map_err(error)?)
        }
        #[cfg(feature = "datetime_mod")]
        "std::datetime::utc_ymd_hms" => {
            let year = i32::try_from(int!()).map_err(|_| "year out of range".to_string())?;
            let month = u32::try_from(int!()).map_err(|_| "month out of range".to_string())?;
            let day = u32::try_from(int!()).map_err(|_| "day out of range".to_string())?;
            let hour = u32::try_from(int!()).map_err(|_| "hour out of range".to_string())?;
            let minute = u32::try_from(int!()).map_err(|_| "minute out of range".to_string())?;
            let second = u32::try_from(int!()).map_err(|_| "second out of range".to_string())?;
            Value::Int(stdlib::datetime_mod::utc_ymd_hms(
                year, month, day, hour, minute, second,
            ))
        }
        // Phase 34's datetime_ext_mod provides these same 8 names; when both
        // features are on, datetime_ext wins and these legacy arms would be
        // unreachable duplicates — hence the not(datetime_ext_mod) gate.
        #[cfg(all(feature = "datetime_mod", not(feature = "datetime_ext_mod")))]
        "std::datetime::add_seconds" => {
            let ts = int!();
            let s = int!();
            Value::Int(stdlib::datetime_mod::add_seconds(ts, s))
        }
        #[cfg(feature = "datetime_mod")]
        "std::datetime::add_days" => {
            let ts = int!();
            let d = int!();
            Value::Int(stdlib::datetime_mod::add_days(ts, d))
        }
        #[cfg(all(feature = "datetime_mod", not(feature = "datetime_ext_mod")))]
        "std::datetime::diff_seconds" => {
            let a = int!();
            let b = int!();
            Value::Int(stdlib::datetime_mod::diff_seconds(a, b))
        }
        #[cfg(all(feature = "datetime_mod", not(feature = "datetime_ext_mod")))]
        "std::datetime::year" => {
            Value::Int(stdlib::datetime_mod::year(int!()).map_err(error)? as i64)
        }
        #[cfg(all(feature = "datetime_mod", not(feature = "datetime_ext_mod")))]
        "std::datetime::month" => {
            Value::Int(stdlib::datetime_mod::month(int!()).map_err(error)? as i64)
        }
        #[cfg(all(feature = "datetime_mod", not(feature = "datetime_ext_mod")))]
        "std::datetime::day" => {
            Value::Int(stdlib::datetime_mod::day(int!()).map_err(error)? as i64)
        }
        #[cfg(all(feature = "datetime_mod", not(feature = "datetime_ext_mod")))]
        "std::datetime::hour" => {
            Value::Int(stdlib::datetime_mod::hour(int!()).map_err(error)? as i64)
        }
        #[cfg(all(feature = "datetime_mod", not(feature = "datetime_ext_mod")))]
        "std::datetime::minute" => {
            Value::Int(stdlib::datetime_mod::minute(int!()).map_err(error)? as i64)
        }
        #[cfg(all(feature = "datetime_mod", not(feature = "datetime_ext_mod")))]
        "std::datetime::second" => {
            Value::Int(stdlib::datetime_mod::second(int!()).map_err(error)? as i64)
        }
        #[cfg(feature = "datetime_mod")]
        "std::datetime::weekday" => {
            Value::Int(stdlib::datetime_mod::weekday(int!()).map_err(error)? as i64)
        }
        #[cfg(feature = "datetime_mod")]
        "std::datetime::format_offset" => {
            let ts = int!();
            let fmt = string!();
            let offset =
                i32::try_from(int!()).map_err(|_| "offset minutes out of range".to_string())?;
            Value::Str(stdlib::datetime_mod::format_offset(ts, &fmt, offset).map_err(error)?)
        }

        // ---------------- Phase 1: url ----------------
        #[cfg(feature = "url_mod")]
        "std::url::is_valid" => Value::Bool(stdlib::url_mod::is_valid(&string!())),
        #[cfg(feature = "url_mod")]
        "std::url::scheme" => Value::Str(stdlib::url_mod::scheme(&string!()).map_err(error)?),
        #[cfg(feature = "url_mod")]
        "std::url::host" => Value::Str(stdlib::url_mod::host(&string!()).map_err(error)?),
        #[cfg(feature = "url_mod")]
        "std::url::port" => Value::Int(
            stdlib::url_mod::port(&string!())
                .map_err(error)?
                .map(i64::from)
                .unwrap_or(-1),
        ),
        #[cfg(feature = "url_mod")]
        "std::url::path" => Value::Str(stdlib::url_mod::path(&string!()).map_err(error)?),
        #[cfg(feature = "url_mod")]
        "std::url::query" => Value::Str(stdlib::url_mod::query(&string!()).map_err(error)?),
        #[cfg(feature = "url_mod")]
        "std::url::fragment" => Value::Str(stdlib::url_mod::fragment(&string!()).map_err(error)?),
        #[cfg(feature = "url_mod")]
        "std::url::parse_query" => Value::Map(
            stdlib::url_mod::parse_query(&string!())
                .into_iter()
                .map(|(k, v)| (k, Value::Str(v)))
                .collect(),
        ),
        #[cfg(feature = "url_mod")]
        "std::url::build_query" => {
            let pairs = array!()
                .into_iter()
                .map(|item| {
                    let mut tuple = expect_array(item)?;
                    if tuple.len() != 2 {
                        return Err("build_query expects an array of [key, value] pairs".into());
                    }
                    let value = expect_string(tuple.remove(1))?;
                    let key = expect_string(tuple.remove(0))?;
                    Ok::<(String, String), String>((key, value))
                })
                .collect::<Result<Vec<_>, String>>()?;
            Value::Str(stdlib::url_mod::build_query(&pairs))
        }
        #[cfg(feature = "url_mod")]
        "std::url::join" => {
            let base = string!();
            let rel = string!();
            Value::Str(stdlib::url_mod::join(&base, &rel).map_err(error)?)
        }

        // ---------------- Phase 2: compress ----------------
        #[cfg(feature = "compress_mod")]
        "std::compress::gzip_encode" => {
            let data = bytes!();
            let lvl = i32::try_from(int!()).unwrap_or(-1);
            Value::Bytes(stdlib::compress_mod::gzip_encode(&data, lvl).map_err(error)?)
        }
        #[cfg(feature = "compress_mod")]
        "std::compress::gzip_decode" => {
            let data = bytes!();
            Value::Bytes(stdlib::compress_mod::gzip_decode(&data).map_err(error)?)
        }
        #[cfg(feature = "compress_mod")]
        "std::compress::zlib_encode" => {
            let data = bytes!();
            let lvl = i32::try_from(int!()).unwrap_or(-1);
            Value::Bytes(stdlib::compress_mod::zlib_encode(&data, lvl).map_err(error)?)
        }
        #[cfg(feature = "compress_mod")]
        "std::compress::zlib_decode" => {
            let data = bytes!();
            Value::Bytes(stdlib::compress_mod::zlib_decode(&data).map_err(error)?)
        }
        #[cfg(feature = "compress_mod")]
        "std::compress::deflate_encode" => {
            let data = bytes!();
            let lvl = i32::try_from(int!()).unwrap_or(-1);
            Value::Bytes(stdlib::compress_mod::deflate_encode(&data, lvl).map_err(error)?)
        }
        #[cfg(feature = "compress_mod")]
        "std::compress::deflate_decode" => {
            let data = bytes!();
            Value::Bytes(stdlib::compress_mod::deflate_decode(&data).map_err(error)?)
        }
        #[cfg(feature = "compress_mod")]
        "std::compress::zstd_encode" => {
            let data = bytes!();
            let lvl = i32::try_from(int!()).unwrap_or(0);
            Value::Bytes(stdlib::compress_mod::zstd_encode(&data, lvl).map_err(error)?)
        }
        #[cfg(feature = "compress_mod")]
        "std::compress::zstd_decode" => {
            let data = bytes!();
            Value::Bytes(stdlib::compress_mod::zstd_decode(&data).map_err(error)?)
        }

        // ---------------- Phase 2: archive ----------------
        #[cfg(feature = "archive_mod")]
        "std::archive::tar_pack" => {
            let entries = value_to_archive_entries(take!())?;
            Value::Bytes(stdlib::archive_mod::tar_pack(&entries).map_err(error)?)
        }
        #[cfg(feature = "archive_mod")]
        "std::archive::tar_unpack" => {
            let data = bytes!();
            archive_entries_to_value(stdlib::archive_mod::tar_unpack(&data).map_err(error)?)
        }
        #[cfg(feature = "archive_mod")]
        "std::archive::zip_pack" => {
            let entries = value_to_archive_entries(take!())?;
            Value::Bytes(stdlib::archive_mod::zip_pack(&entries).map_err(error)?)
        }
        #[cfg(feature = "archive_mod")]
        "std::archive::zip_unpack" => {
            let data = bytes!();
            archive_entries_to_value(stdlib::archive_mod::zip_unpack(&data).map_err(error)?)
        }
        #[cfg(feature = "archive_mod")]
        "std::archive::zip_list" => {
            let data = bytes!();
            Value::Array(
                stdlib::archive_mod::zip_list(&data)
                    .map_err(error)?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }

        // ---------------- Phase 2: yaml ----------------
        #[cfg(feature = "yaml_mod")]
        "std::yaml::parse" => from_json(stdlib::yaml_mod::parse(&string!()).map_err(error)?)?,
        #[cfg(feature = "yaml_mod")]
        "std::yaml::stringify" => {
            Value::Str(stdlib::yaml_mod::stringify(&to_json(take!())?).map_err(error)?)
        }
        #[cfg(feature = "yaml_mod")]
        "std::yaml::parse_multi" => Value::Array(
            stdlib::yaml_mod::parse_multi(&string!())
                .map_err(error)?
                .into_iter()
                .map(from_json)
                .collect::<Result<_, _>>()?,
        ),

        // ---------------- Phase 2: xml ----------------
        #[cfg(feature = "xml_mod")]
        "std::xml::parse" => from_json(stdlib::xml_mod::parse(&string!()).map_err(error)?)?,
        #[cfg(feature = "xml_mod")]
        "std::xml::stringify" => {
            Value::Str(stdlib::xml_mod::stringify(&to_json(take!())?).map_err(error)?)
        }
        #[cfg(feature = "xml_mod")]
        "std::xml::escape_text" => Value::Str(stdlib::xml_mod::escape_text(&string!())),
        #[cfg(feature = "xml_mod")]
        "std::xml::escape_attr" => Value::Str(stdlib::xml_mod::escape_attr(&string!())),

        // ---------------- Phase 3: http_full ----------------
        #[cfg(feature = "http_full_mod")]
        "std::http_full::request" => {
            let method = string!();
            let url = string!();
            let headers_map = expect_map(take!())?;
            let body = bytes!();
            let options_map = expect_map(take!())?;
            let mut options = stdlib::http_full_mod::Options::default();
            options.headers = headers_map
                .into_iter()
                .map(|(k, v)| Ok::<_, String>((k, expect_string(v)?)))
                .collect::<Result<_, _>>()?;
            build_http_full_options(&mut options, options_map)?;
            let response = call_http_full(&method, &url, &body, &options)?;
            http_full_response_to_value(response)
        }
        #[cfg(feature = "http_full_mod")]
        "std::http_full::get_json" => {
            let url = string!();
            let headers_map = expect_map(take!())?;
            let options_map = expect_map(take!())?;
            let mut options = stdlib::http_full_mod::Options::default();
            options.headers = headers_map
                .into_iter()
                .map(|(k, v)| Ok::<_, String>((k, expect_string(v)?)))
                .collect::<Result<_, _>>()?;
            build_http_full_options(&mut options, options_map)?;
            from_json(stdlib::http_full_mod::get_json(&url, &options).map_err(error)?)?
        }
        #[cfg(feature = "http_full_mod")]
        "std::http_full::post_json" => {
            let url = string!();
            let body = to_json(take!())?;
            let headers_map = expect_map(take!())?;
            let options_map = expect_map(take!())?;
            let mut options = stdlib::http_full_mod::Options::default();
            options.headers = headers_map
                .into_iter()
                .map(|(k, v)| Ok::<_, String>((k, expect_string(v)?)))
                .collect::<Result<_, _>>()?;
            build_http_full_options(&mut options, options_map)?;
            from_json(stdlib::http_full_mod::post_json(&url, &body, &options).map_err(error)?)?
        }
        #[cfg(feature = "http_full_mod")]
        "std::http_full::post_form" => {
            let url = string!();
            let form = array!()
                .into_iter()
                .map(|item| {
                    let mut tuple = expect_array(item)?;
                    if tuple.len() != 2 {
                        return Err("post_form pairs must be [key, value]".to_string());
                    }
                    let value = expect_string(tuple.remove(1))?;
                    let key = expect_string(tuple.remove(0))?;
                    Ok::<(String, String), String>((key, value))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let headers_map = expect_map(take!())?;
            let options_map = expect_map(take!())?;
            let mut options = stdlib::http_full_mod::Options::default();
            options.headers = headers_map
                .into_iter()
                .map(|(k, v)| Ok::<_, String>((k, expect_string(v)?)))
                .collect::<Result<_, _>>()?;
            build_http_full_options(&mut options, options_map)?;
            let response =
                stdlib::http_full_mod::post_form(&url, &form, &options).map_err(error)?;
            http_full_response_to_value(response)
        }

        // ---------------- Phase 3: dns ----------------
        #[cfg(feature = "dns_mod")]
        "std::dns::resolve" => Value::Array(
            stdlib::dns_mod::resolve(&string!())
                .map_err(error)?
                .into_iter()
                .map(Value::Str)
                .collect(),
        ),
        #[cfg(feature = "dns_mod")]
        "std::dns::resolve_ipv4" => Value::Array(
            stdlib::dns_mod::resolve_ipv4(&string!())
                .map_err(error)?
                .into_iter()
                .map(Value::Str)
                .collect(),
        ),
        #[cfg(feature = "dns_mod")]
        "std::dns::resolve_ipv6" => Value::Array(
            stdlib::dns_mod::resolve_ipv6(&string!())
                .map_err(error)?
                .into_iter()
                .map(Value::Str)
                .collect(),
        ),
        #[cfg(feature = "dns_mod")]
        "std::dns::resolve_mx" => Value::Array(
            stdlib::dns_mod::resolve_mx(&string!())
                .map_err(error)?
                .into_iter()
                .map(Value::Str)
                .collect(),
        ),
        #[cfg(feature = "dns_mod")]
        "std::dns::resolve_txt" => Value::Array(
            stdlib::dns_mod::resolve_txt(&string!())
                .map_err(error)?
                .into_iter()
                .map(Value::Str)
                .collect(),
        ),
        #[cfg(feature = "dns_mod")]
        "std::dns::resolve_cname" => Value::Array(
            stdlib::dns_mod::resolve_cname(&string!())
                .map_err(error)?
                .into_iter()
                .map(Value::Str)
                .collect(),
        ),
        #[cfg(feature = "dns_mod")]
        "std::dns::reverse" => Value::Array(
            stdlib::dns_mod::reverse(&string!())
                .map_err(error)?
                .into_iter()
                .map(Value::Str)
                .collect(),
        ),

        // ---------------- Phase 3: email ----------------
        #[cfg(feature = "email_mod")]
        "std::email::send_simple" => {
            let host = string!();
            let port = u16::try_from(int!()).map_err(|_| "port out of range".to_string())?;
            let user = string!();
            let pass = string!();
            let from = string!();
            let to = string!();
            let subject = string!();
            let body = string!();
            Value::Str(
                stdlib::email_mod::send_simple(
                    &host, port, &user, &pass, &from, &to, &subject, &body,
                )
                .map_err(error)?,
            )
        }
        #[cfg(feature = "email_mod")]
        "std::email::send_html" => {
            let host = string!();
            let port = u16::try_from(int!()).map_err(|_| "port out of range".to_string())?;
            let user = string!();
            let pass = string!();
            let from = string!();
            let to = string!();
            let subject = string!();
            let text = string!();
            let html = string!();
            Value::Str(
                stdlib::email_mod::send_html(
                    &host, port, &user, &pass, &from, &to, &subject, &text, &html,
                )
                .map_err(error)?,
            )
        }
        #[cfg(feature = "email_mod")]
        "std::email::send_with_attachment" => {
            let host = string!();
            let port = u16::try_from(int!()).map_err(|_| "port out of range".to_string())?;
            let user = string!();
            let pass = string!();
            let from = string!();
            let to = string!();
            let subject = string!();
            let html = string!();
            let filename = string!();
            let mime = string!();
            let bytes = bytes!();
            Value::Str(
                stdlib::email_mod::send_with_attachment(
                    &host, port, &user, &pass, &from, &to, &subject, &html, &filename, &mime,
                    &bytes,
                )
                .map_err(error)?,
            )
        }

        // ---------------- Phase 4: crypto ----------------
        #[cfg(feature = "crypto_mod")]
        "std::crypto::generate_key_32" => Value::Bytes(stdlib::crypto_mod::generate_key_32()),
        #[cfg(feature = "crypto_mod")]
        "std::crypto::generate_nonce" => Value::Bytes(stdlib::crypto_mod::generate_nonce()),
        #[cfg(feature = "crypto_mod")]
        "std::crypto::chacha20_encrypt" => {
            let key = bytes!();
            let nonce = bytes!();
            let pt = bytes!();
            let aad = bytes!();
            Value::Bytes(
                stdlib::crypto_mod::chacha20_encrypt(&key, &nonce, &pt, &aad).map_err(error)?,
            )
        }
        #[cfg(feature = "crypto_mod")]
        "std::crypto::chacha20_decrypt" => {
            let key = bytes!();
            let nonce = bytes!();
            let ct = bytes!();
            let aad = bytes!();
            Value::Bytes(
                stdlib::crypto_mod::chacha20_decrypt(&key, &nonce, &ct, &aad).map_err(error)?,
            )
        }
        #[cfg(feature = "crypto_mod")]
        "std::crypto::chacha20_seal" => {
            let key = bytes!();
            let pt = bytes!();
            let aad = bytes!();
            Value::Bytes(stdlib::crypto_mod::chacha20_seal(&key, &pt, &aad).map_err(error)?)
        }
        #[cfg(feature = "crypto_mod")]
        "std::crypto::chacha20_open" => {
            let key = bytes!();
            let sealed = bytes!();
            let aad = bytes!();
            Value::Bytes(stdlib::crypto_mod::chacha20_open(&key, &sealed, &aad).map_err(error)?)
        }
        #[cfg(feature = "crypto_mod")]
        "std::crypto::aes_gcm_encrypt" => {
            let key = bytes!();
            let nonce = bytes!();
            let pt = bytes!();
            let aad = bytes!();
            Value::Bytes(
                stdlib::crypto_mod::aes_gcm_encrypt(&key, &nonce, &pt, &aad).map_err(error)?,
            )
        }
        #[cfg(feature = "crypto_mod")]
        "std::crypto::aes_gcm_decrypt" => {
            let key = bytes!();
            let nonce = bytes!();
            let ct = bytes!();
            let aad = bytes!();
            Value::Bytes(
                stdlib::crypto_mod::aes_gcm_decrypt(&key, &nonce, &ct, &aad).map_err(error)?,
            )
        }
        #[cfg(feature = "crypto_mod")]
        "std::crypto::aes_gcm_seal" => {
            let key = bytes!();
            let pt = bytes!();
            let aad = bytes!();
            Value::Bytes(stdlib::crypto_mod::aes_gcm_seal(&key, &pt, &aad).map_err(error)?)
        }
        #[cfg(feature = "crypto_mod")]
        "std::crypto::aes_gcm_open" => {
            let key = bytes!();
            let sealed = bytes!();
            let aad = bytes!();
            Value::Bytes(stdlib::crypto_mod::aes_gcm_open(&key, &sealed, &aad).map_err(error)?)
        }

        // ---------------- Phase 4: password ----------------
        #[cfg(feature = "password_mod")]
        "std::password::hash_argon2" => {
            Value::Str(stdlib::password_mod::hash_argon2(&string!()).map_err(error)?)
        }
        #[cfg(feature = "password_mod")]
        "std::password::verify_argon2" => {
            let hash = string!();
            let pass = string!();
            Value::Bool(stdlib::password_mod::verify_argon2(&hash, &pass).map_err(error)?)
        }
        #[cfg(feature = "password_mod")]
        "std::password::hash_bcrypt" => {
            let pass = string!();
            let cost = u32::try_from(int!()).map_err(|_| "bcrypt cost out of range".to_string())?;
            Value::Str(stdlib::password_mod::hash_bcrypt(&pass, cost).map_err(error)?)
        }
        #[cfg(feature = "password_mod")]
        "std::password::verify_bcrypt" => {
            let hash = string!();
            let pass = string!();
            Value::Bool(stdlib::password_mod::verify_bcrypt(&hash, &pass).map_err(error)?)
        }

        // ---------------- Phase 4: JWT ----------------
        #[cfg(feature = "jwt_mod")]
        "std::jwt::sign_hs256" => {
            let claims = to_json(take!())?;
            let secret = bytes!();
            Value::Str(stdlib::jwt_mod::sign_hs256(&claims, &secret).map_err(error)?)
        }
        #[cfg(feature = "jwt_mod")]
        "std::jwt::verify_hs256" => {
            let token = string!();
            let secret = bytes!();
            let aud = string!();
            let iss = string!();
            let aud_opt = if aud.is_empty() {
                None
            } else {
                Some(aud.as_str())
            };
            let iss_opt = if iss.is_empty() {
                None
            } else {
                Some(iss.as_str())
            };
            from_json(
                stdlib::jwt_mod::verify_hs256(&token, &secret, aud_opt, iss_opt).map_err(error)?,
            )?
        }
        #[cfg(feature = "jwt_mod")]
        "std::jwt::sign_rs256" => {
            let claims = to_json(take!())?;
            let pem = bytes!();
            Value::Str(stdlib::jwt_mod::sign_rs256(&claims, &pem).map_err(error)?)
        }
        #[cfg(feature = "jwt_mod")]
        "std::jwt::verify_rs256" => {
            let token = string!();
            let pem = bytes!();
            let aud = string!();
            let iss = string!();
            let aud_opt = if aud.is_empty() {
                None
            } else {
                Some(aud.as_str())
            };
            let iss_opt = if iss.is_empty() {
                None
            } else {
                Some(iss.as_str())
            };
            from_json(
                stdlib::jwt_mod::verify_rs256(&token, &pem, aud_opt, iss_opt).map_err(error)?,
            )?
        }
        #[cfg(feature = "jwt_mod")]
        "std::jwt::peek_header" => {
            from_json(stdlib::jwt_mod::peek_header(&string!()).map_err(error)?)?
        }

        // ---------------- Phase 5: Termux / Android ----------------
        #[cfg(feature = "termux_mod")]
        "std::termux::is_available" => Value::Bool(stdlib::termux_mod::is_available()),
        #[cfg(feature = "termux_mod")]
        "std::termux::battery_status" => {
            from_json(stdlib::termux_mod::battery_status().map_err(error)?)?
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::wifi_info" => from_json(stdlib::termux_mod::wifi_info().map_err(error)?)?,
        #[cfg(feature = "termux_mod")]
        "std::termux::telephony_info" => {
            from_json(stdlib::termux_mod::telephony_info().map_err(error)?)?
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::location" => {
            let provider = string!();
            let request = string!();
            from_json(stdlib::termux_mod::location(&provider, &request).map_err(error)?)?
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::sensor_list" => Value::Array(
            stdlib::termux_mod::sensor_list()
                .map_err(error)?
                .into_iter()
                .map(Value::Str)
                .collect(),
        ),
        #[cfg(feature = "termux_mod")]
        "std::termux::sensor_read" => {
            from_json(stdlib::termux_mod::sensor_read(&string!()).map_err(error)?)?
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::clipboard_get" => {
            Value::Str(stdlib::termux_mod::clipboard_get().map_err(error)?)
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::clipboard_set" => {
            stdlib::termux_mod::clipboard_set(&string!()).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::vibrate" => {
            let ms = int!();
            let force = expect_bool(take!())?;
            stdlib::termux_mod::vibrate(std::time::Duration::from_millis(ms.max(0) as u64), force)
                .map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::torch" => {
            let on = expect_bool(take!())?;
            stdlib::termux_mod::torch(on).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::toast" => {
            stdlib::termux_mod::toast(&string!()).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::notify" => {
            let title = string!();
            let content = string!();
            let id = int!();
            stdlib::termux_mod::notify(&title, &content, id).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::notify_remove" => {
            stdlib::termux_mod::notify_remove(int!()).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::tts_speak" => {
            stdlib::termux_mod::tts_speak(&string!()).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::sms_list" => from_json(stdlib::termux_mod::sms_list(int!()).map_err(error)?)?,
        #[cfg(feature = "termux_mod")]
        "std::termux::sms_send" => {
            let recipient = string!();
            let message = string!();
            stdlib::termux_mod::sms_send(&recipient, &message).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::contacts" => from_json(stdlib::termux_mod::contacts().map_err(error)?)?,
        #[cfg(feature = "termux_mod")]
        "std::termux::camera_info" => from_json(stdlib::termux_mod::camera_info().map_err(error)?)?,
        #[cfg(feature = "termux_mod")]
        "std::termux::camera_photo" => {
            let camera_id = string!();
            let output_path = string!();
            stdlib::termux_mod::camera_photo(&camera_id, &output_path).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::brightness" => {
            stdlib::termux_mod::brightness(int!()).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::dialog" => {
            let dialog_type = string!();
            let title = string!();
            from_json(stdlib::termux_mod::dialog(&dialog_type, &title).map_err(error)?)?
        }
        #[cfg(feature = "termux_mod")]
        "std::termux::share" => {
            stdlib::termux_mod::share(&string!()).map_err(error)?;
            Value::Nil
        }

        // ---------------- Phase 6: terminal (crossterm) ----------------
        #[cfg(feature = "term_mod")]
        "std::term::print_colored" => {
            let color = string!();
            let text = string!();
            stdlib::term_mod::print_colored(&color, &text).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "term_mod")]
        "std::term::print_styled" => {
            let fg = string!();
            let bg = string!();
            let text = string!();
            stdlib::term_mod::print_styled(&fg, &bg, &text).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "term_mod")]
        "std::term::print_attr" => {
            let attr = string!();
            let text = string!();
            stdlib::term_mod::print_attr(&attr, &text).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "term_mod")]
        "std::term::clear_screen" => {
            stdlib::term_mod::clear_screen().map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "term_mod")]
        "std::term::clear_line" => {
            stdlib::term_mod::clear_line().map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "term_mod")]
        "std::term::move_to" => {
            let column = u16::try_from(int!()).map_err(|_| "column out of range".to_string())?;
            let row = u16::try_from(int!()).map_err(|_| "row out of range".to_string())?;
            stdlib::term_mod::move_to(column, row).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "term_mod")]
        "std::term::hide_cursor" => {
            stdlib::term_mod::hide_cursor().map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "term_mod")]
        "std::term::show_cursor" => {
            stdlib::term_mod::show_cursor().map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "term_mod")]
        "std::term::size" => {
            let (columns, rows) = stdlib::term_mod::size().map_err(error)?;
            Value::Array(vec![Value::Int(columns as i64), Value::Int(rows as i64)])
        }
        #[cfg(feature = "term_mod")]
        "std::term::flush" => {
            stdlib::term_mod::flush().map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "term_mod")]
        "std::term::enter_alt_screen" => {
            stdlib::term_mod::enter_alt_screen().map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "term_mod")]
        "std::term::leave_alt_screen" => {
            stdlib::term_mod::leave_alt_screen().map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "term_mod")]
        "std::term::enable_raw" => {
            stdlib::term_mod::enable_raw().map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "term_mod")]
        "std::term::disable_raw" => {
            stdlib::term_mod::disable_raw().map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "term_mod")]
        "std::term::read_key" => {
            let timeout =
                u64::try_from(int!()).map_err(|_| "timeout must be nonnegative".to_string())?;
            Value::Str(stdlib::term_mod::read_key(timeout).map_err(error)?)
        }

        // ---------------- Phase 6: readline (rustyline) ----------------
        #[cfg(feature = "readline_mod")]
        "std::readline::prompt" => {
            Value::Str(stdlib::readline_mod::prompt(&string!()).map_err(error)?)
        }
        #[cfg(feature = "readline_mod")]
        "std::readline::prompt_with_history" => {
            Value::Str(stdlib::readline_mod::prompt_with_history(&string!()).map_err(error)?)
        }
        #[cfg(feature = "readline_mod")]
        "std::readline::prompt_persistent" => {
            let p = string!();
            let path = string!();
            Value::Str(stdlib::readline_mod::prompt_persistent(&p, &path).map_err(error)?)
        }
        #[cfg(feature = "readline_mod")]
        "std::readline::prompt_secret" => {
            Value::Str(stdlib::readline_mod::prompt_secret(&string!()).map_err(error)?)
        }

        // ---------------- Phase 6: progress (indicatif) ----------------
        #[cfg(feature = "progress_mod")]
        "std::progress::bar_new" => {
            let total =
                u64::try_from(int!()).map_err(|_| "total must be nonnegative".to_string())?;
            Value::Int(stdlib::progress_mod::bar_new(total).map_err(error)?)
        }
        #[cfg(feature = "progress_mod")]
        "std::progress::spinner_new" => {
            Value::Int(stdlib::progress_mod::spinner_new().map_err(error)?)
        }
        #[cfg(feature = "progress_mod")]
        "std::progress::set_message" => {
            let id = int!();
            let message = string!();
            stdlib::progress_mod::set_message(id, &message).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "progress_mod")]
        "std::progress::set_position" => {
            let id = int!();
            let position =
                u64::try_from(int!()).map_err(|_| "position must be nonnegative".to_string())?;
            stdlib::progress_mod::set_position(id, position);
            Value::Nil
        }
        #[cfg(feature = "progress_mod")]
        "std::progress::increment" => {
            let id = int!();
            let delta =
                u64::try_from(int!()).map_err(|_| "delta must be nonnegative".to_string())?;
            stdlib::progress_mod::increment(id, delta);
            Value::Nil
        }
        #[cfg(feature = "progress_mod")]
        "std::progress::finish" => {
            let id = int!();
            let message = string!();
            stdlib::progress_mod::finish(id, &message).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "progress_mod")]
        "std::progress::abandon" => {
            stdlib::progress_mod::abandon(int!());
            Value::Nil
        }

        // ---------------- Phase 7: images (image crate) ----------------
        #[cfg(feature = "image_mod")]
        "std::image::load" => Value::Int(stdlib::image_mod::load(&string!()).map_err(error)?),
        #[cfg(feature = "image_mod")]
        "std::image::load_bytes" => {
            Value::Int(stdlib::image_mod::load_bytes(&bytes!()).map_err(error)?)
        }
        #[cfg(feature = "image_mod")]
        "std::image::from_rgba" => {
            let width = u32::try_from(int!()).map_err(|_| "width out of range".to_string())?;
            let height = u32::try_from(int!()).map_err(|_| "height out of range".to_string())?;
            let data = bytes!();
            Value::Int(stdlib::image_mod::from_rgba(width, height, &data).map_err(error)?)
        }
        #[cfg(feature = "image_mod")]
        "std::image::save" => {
            let handle = int!();
            let path = string!();
            stdlib::image_mod::save(handle, &path).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "image_mod")]
        "std::image::encode" => {
            let handle = int!();
            let format = string!();
            Value::Bytes(stdlib::image_mod::encode(handle, &format).map_err(error)?)
        }
        #[cfg(feature = "image_mod")]
        "std::image::width" => Value::Int(stdlib::image_mod::width(int!()).map_err(error)? as i64),
        #[cfg(feature = "image_mod")]
        "std::image::height" => {
            Value::Int(stdlib::image_mod::height(int!()).map_err(error)? as i64)
        }
        #[cfg(feature = "image_mod")]
        "std::image::color_type" => {
            Value::Str(stdlib::image_mod::color_type(int!()).map_err(error)?)
        }
        #[cfg(feature = "image_mod")]
        "std::image::resize" => {
            let handle = int!();
            let width = u32::try_from(int!()).map_err(|_| "width out of range".to_string())?;
            let height = u32::try_from(int!()).map_err(|_| "height out of range".to_string())?;
            let filter = string!();
            Value::Int(stdlib::image_mod::resize(handle, width, height, &filter).map_err(error)?)
        }
        #[cfg(feature = "image_mod")]
        "std::image::resize_exact" => {
            let handle = int!();
            let width = u32::try_from(int!()).map_err(|_| "width out of range".to_string())?;
            let height = u32::try_from(int!()).map_err(|_| "height out of range".to_string())?;
            let filter = string!();
            Value::Int(
                stdlib::image_mod::resize_exact(handle, width, height, &filter).map_err(error)?,
            )
        }
        #[cfg(feature = "image_mod")]
        "std::image::thumbnail" => {
            let handle = int!();
            let width = u32::try_from(int!()).map_err(|_| "width out of range".to_string())?;
            let height = u32::try_from(int!()).map_err(|_| "height out of range".to_string())?;
            Value::Int(stdlib::image_mod::thumbnail(handle, width, height).map_err(error)?)
        }
        #[cfg(feature = "image_mod")]
        "std::image::crop" => {
            let handle = int!();
            let x = u32::try_from(int!()).map_err(|_| "x out of range".to_string())?;
            let y = u32::try_from(int!()).map_err(|_| "y out of range".to_string())?;
            let width = u32::try_from(int!()).map_err(|_| "width out of range".to_string())?;
            let height = u32::try_from(int!()).map_err(|_| "height out of range".to_string())?;
            Value::Int(stdlib::image_mod::crop(handle, x, y, width, height).map_err(error)?)
        }
        #[cfg(feature = "image_mod")]
        "std::image::grayscale" => Value::Int(stdlib::image_mod::grayscale(int!()).map_err(error)?),
        #[cfg(feature = "image_mod")]
        "std::image::blur" => {
            let handle = int!();
            let sigma = float!();
            if !sigma.is_finite() || sigma < f32::MIN as f64 || sigma > f32::MAX as f64 {
                return Err("blur sigma out of range".to_string());
            }
            Value::Int(stdlib::image_mod::blur(handle, sigma as f32).map_err(error)?)
        }
        #[cfg(feature = "image_mod")]
        "std::image::brighten" => {
            let handle = int!();
            let value = i32::try_from(int!()).map_err(|_| "brightness out of range".to_string())?;
            Value::Int(stdlib::image_mod::brighten(handle, value).map_err(error)?)
        }
        #[cfg(feature = "image_mod")]
        "std::image::rotate90" => Value::Int(stdlib::image_mod::rotate90(int!()).map_err(error)?),
        #[cfg(feature = "image_mod")]
        "std::image::rotate180" => Value::Int(stdlib::image_mod::rotate180(int!()).map_err(error)?),
        #[cfg(feature = "image_mod")]
        "std::image::rotate270" => Value::Int(stdlib::image_mod::rotate270(int!()).map_err(error)?),
        #[cfg(feature = "image_mod")]
        "std::image::flip_horizontal" => {
            Value::Int(stdlib::image_mod::flip_horizontal(int!()).map_err(error)?)
        }
        #[cfg(feature = "image_mod")]
        "std::image::flip_vertical" => {
            Value::Int(stdlib::image_mod::flip_vertical(int!()).map_err(error)?)
        }
        #[cfg(feature = "image_mod")]
        "std::image::close" => {
            stdlib::image_mod::close(int!());
            Value::Nil
        }

        // ---------------- Phase 7: QR codes (qrcode crate) ----------------
        #[cfg(feature = "qrcode_mod")]
        "std::qrcode::to_ascii" => {
            let text = string!();
            let level = string!();
            let dark = string!();
            let light = string!();
            Value::Str(stdlib::qrcode_mod::to_ascii(&text, &level, &dark, &light).map_err(error)?)
        }
        #[cfg(feature = "qrcode_mod")]
        "std::qrcode::to_unicode" => {
            let text = string!();
            let level = string!();
            Value::Str(stdlib::qrcode_mod::to_unicode(&text, &level).map_err(error)?)
        }
        #[cfg(feature = "qrcode_mod")]
        "std::qrcode::to_svg" => {
            let text = string!();
            let level = string!();
            let module_pixels =
                u32::try_from(int!()).map_err(|_| "module_pixels out of range".to_string())?;
            Value::Bytes(stdlib::qrcode_mod::to_svg(&text, &level, module_pixels).map_err(error)?)
        }
        #[cfg(feature = "qrcode_mod")]
        "std::qrcode::to_png" => {
            let text = string!();
            let level = string!();
            let side_pixels =
                u32::try_from(int!()).map_err(|_| "side_pixels out of range".to_string())?;
            Value::Bytes(stdlib::qrcode_mod::to_png(&text, &level, side_pixels).map_err(error)?)
        }
        #[cfg(feature = "qrcode_mod")]
        "std::qrcode::save_png" => {
            let text = string!();
            let level = string!();
            let side_pixels =
                u32::try_from(int!()).map_err(|_| "side_pixels out of range".to_string())?;
            let path = string!();
            stdlib::qrcode_mod::save_png(&text, &level, side_pixels, &path).map_err(error)?;
            Value::Nil
        }

        // ---------------- Phase 8: procfs (sysinfo) ----------------
        #[cfg(feature = "procfs_mod")]
        "std::procfs::hostname" => Value::Str(stdlib::procfs_mod::hostname()),
        #[cfg(feature = "procfs_mod")]
        "std::procfs::kernel" => Value::Str(stdlib::procfs_mod::kernel()),
        #[cfg(feature = "procfs_mod")]
        "std::procfs::os_name" => Value::Str(stdlib::procfs_mod::os_name()),
        #[cfg(feature = "procfs_mod")]
        "std::procfs::os_version" => Value::Str(stdlib::procfs_mod::os_version()),
        #[cfg(feature = "procfs_mod")]
        "std::procfs::uptime" => Value::Int(stdlib::procfs_mod::uptime() as i64),
        #[cfg(feature = "procfs_mod")]
        "std::procfs::cpu_usage" => Value::Float(stdlib::procfs_mod::cpu_usage() as f64),
        #[cfg(feature = "procfs_mod")]
        "std::procfs::cpu_count" => Value::Int(stdlib::procfs_mod::cpu_count() as i64),
        #[cfg(feature = "procfs_mod")]
        "std::procfs::cpus" => from_json(stdlib::procfs_mod::cpus())?,
        #[cfg(feature = "procfs_mod")]
        "std::procfs::total_memory" => Value::Int(stdlib::procfs_mod::total_memory() as i64),
        #[cfg(feature = "procfs_mod")]
        "std::procfs::used_memory" => Value::Int(stdlib::procfs_mod::used_memory() as i64),
        #[cfg(feature = "procfs_mod")]
        "std::procfs::available_memory" => {
            Value::Int(stdlib::procfs_mod::available_memory() as i64)
        }
        #[cfg(feature = "procfs_mod")]
        "std::procfs::total_swap" => Value::Int(stdlib::procfs_mod::total_swap() as i64),
        #[cfg(feature = "procfs_mod")]
        "std::procfs::used_swap" => Value::Int(stdlib::procfs_mod::used_swap() as i64),
        #[cfg(feature = "procfs_mod")]
        "std::procfs::load_average" => from_json(stdlib::procfs_mod::load_average())?,
        #[cfg(feature = "procfs_mod")]
        "std::procfs::process_count" => Value::Int(stdlib::procfs_mod::process_count() as i64),
        #[cfg(feature = "procfs_mod")]
        "std::procfs::top_processes" => {
            let limit = usize::try_from(int!()).unwrap_or(10);
            from_json(stdlib::procfs_mod::top_processes(limit))?
        }
        #[cfg(feature = "procfs_mod")]
        "std::procfs::disks" => from_json(stdlib::procfs_mod::disks())?,
        #[cfg(feature = "procfs_mod")]
        "std::procfs::networks" => from_json(stdlib::procfs_mod::networks())?,

        // ---------------- Phase 8: fswatch (notify) ----------------
        #[cfg(feature = "fswatch_mod")]
        "std::fswatch::watch_once" => {
            let path = string!();
            let timeout = u64::try_from(int!()).unwrap_or(1000);
            let recursive = expect_bool(take!())?;
            Value::Str(stdlib::fswatch_mod::watch_once(&path, timeout, recursive).map_err(error)?)
        }
        #[cfg(feature = "fswatch_mod")]
        "std::fswatch::open" => {
            let path = string!();
            let recursive = expect_bool(take!())?;
            Value::Int(stdlib::fswatch_mod::open(&path, recursive).map_err(error)?)
        }
        #[cfg(feature = "fswatch_mod")]
        "std::fswatch::next_event" => {
            let handle = int!();
            let timeout = u64::try_from(int!()).unwrap_or(1000);
            Value::Str(stdlib::fswatch_mod::next_event(handle, timeout).map_err(error)?)
        }
        #[cfg(feature = "fswatch_mod")]
        "std::fswatch::close" => {
            stdlib::fswatch_mod::close(int!());
            Value::Nil
        }

        // ------- Phase 8: Unix signals (POSIX: no existen en Windows) -------
        #[cfg(all(feature = "signals_mod", unix))]
        "std::signals::install" => {
            stdlib::signals_mod::install(&string!()).map_err(error)?;
            Value::Nil
        }
        #[cfg(all(feature = "signals_mod", unix))]
        "std::signals::pending" => {
            Value::Int(stdlib::signals_mod::pending(&string!()).map_err(error)? as i64)
        }
        #[cfg(all(feature = "signals_mod", unix))]
        "std::signals::wait_any" => {
            let timeout = u64::try_from(int!()).unwrap_or(1000);
            Value::Str(stdlib::signals_mod::wait_any(timeout).map_err(error)?)
        }

        // ---------------- Phase 9: audio (hound + termux-media) ----------------
        #[cfg(feature = "audio_mod")]
        "std::audio::read_wav" => {
            let path = string!();
            let (samples, sample_rate, channels, bits) =
                stdlib::audio_mod::read_wav(&path).map_err(error)?;
            audio_read_result(samples, sample_rate, channels, bits)
        }
        #[cfg(feature = "audio_mod")]
        "std::audio::read_wav_bytes" => {
            let data = bytes!();
            let (samples, sample_rate, channels, bits) =
                stdlib::audio_mod::read_wav_bytes(&data).map_err(error)?;
            audio_read_result(samples, sample_rate, channels, bits)
        }
        #[cfg(feature = "audio_mod")]
        "std::audio::write_wav" => {
            let path = string!();
            let samples = audio_samples_from_array(array!())?;
            let sample_rate =
                u32::try_from(int!()).map_err(|_| "sample_rate out of range".to_string())?;
            let channels =
                u16::try_from(int!()).map_err(|_| "channels out of range".to_string())?;
            stdlib::audio_mod::write_wav(&path, &samples, sample_rate, channels).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "audio_mod")]
        "std::audio::encode_wav" => {
            let samples = audio_samples_from_array(array!())?;
            let sample_rate =
                u32::try_from(int!()).map_err(|_| "sample_rate out of range".to_string())?;
            let channels =
                u16::try_from(int!()).map_err(|_| "channels out of range".to_string())?;
            Value::Bytes(
                stdlib::audio_mod::encode_wav(&samples, sample_rate, channels).map_err(error)?,
            )
        }
        #[cfg(feature = "audio_mod")]
        "std::audio::sine_wave" => {
            let freq = float!() as f32;
            let duration_ms =
                u32::try_from(int!()).map_err(|_| "duration_ms out of range".to_string())?;
            let sample_rate =
                u32::try_from(int!()).map_err(|_| "sample_rate out of range".to_string())?;
            let amplitude = float!() as f32;
            audio_samples_to_array(stdlib::audio_mod::sine_wave(
                freq,
                duration_ms,
                sample_rate,
                amplitude,
            ))
        }
        #[cfg(feature = "audio_mod")]
        "std::audio::square_wave" => {
            let freq = float!() as f32;
            let duration_ms =
                u32::try_from(int!()).map_err(|_| "duration_ms out of range".to_string())?;
            let sample_rate =
                u32::try_from(int!()).map_err(|_| "sample_rate out of range".to_string())?;
            let amplitude = float!() as f32;
            audio_samples_to_array(stdlib::audio_mod::square_wave(
                freq,
                duration_ms,
                sample_rate,
                amplitude,
            ))
        }
        #[cfg(feature = "audio_mod")]
        "std::audio::saw_wave" => {
            let freq = float!() as f32;
            let duration_ms =
                u32::try_from(int!()).map_err(|_| "duration_ms out of range".to_string())?;
            let sample_rate =
                u32::try_from(int!()).map_err(|_| "sample_rate out of range".to_string())?;
            let amplitude = float!() as f32;
            audio_samples_to_array(stdlib::audio_mod::saw_wave(
                freq,
                duration_ms,
                sample_rate,
                amplitude,
            ))
        }
        #[cfg(feature = "audio_mod")]
        "std::audio::white_noise" => {
            let duration_ms =
                u32::try_from(int!()).map_err(|_| "duration_ms out of range".to_string())?;
            let sample_rate =
                u32::try_from(int!()).map_err(|_| "sample_rate out of range".to_string())?;
            let amplitude = float!() as f32;
            audio_samples_to_array(stdlib::audio_mod::white_noise(
                duration_ms,
                sample_rate,
                amplitude,
            ))
        }
        #[cfg(feature = "audio_mod")]
        "std::audio::is_termux_media_available" => {
            Value::Bool(stdlib::audio_mod::is_termux_media_available())
        }
        #[cfg(feature = "audio_mod")]
        "std::audio::play" => Value::Str(stdlib::audio_mod::play(&string!()).map_err(error)?),
        #[cfg(feature = "audio_mod")]
        "std::audio::pause" => Value::Str(stdlib::audio_mod::pause().map_err(error)?),
        #[cfg(feature = "audio_mod")]
        "std::audio::resume" => Value::Str(stdlib::audio_mod::resume().map_err(error)?),
        #[cfg(feature = "audio_mod")]
        "std::audio::stop" => Value::Str(stdlib::audio_mod::stop().map_err(error)?),
        #[cfg(feature = "audio_mod")]
        "std::audio::info" => Value::Str(stdlib::audio_mod::info().map_err(error)?),
        #[cfg(feature = "audio_mod")]
        "std::audio::record_start" => {
            let path = string!();
            let secs = u32::try_from(int!()).map_err(|_| "seconds out of range".to_string())?;
            Value::Str(stdlib::audio_mod::record_start(&path, secs).map_err(error)?)
        }
        #[cfg(feature = "audio_mod")]
        "std::audio::record_stop" => Value::Str(stdlib::audio_mod::record_stop().map_err(error)?),
        #[cfg(feature = "audio_mod")]
        "std::audio::record_info" => Value::Str(stdlib::audio_mod::record_info().map_err(error)?),

        // ---------------- Phase 10: sled key-value ----------------
        #[cfg(feature = "kv_mod")]
        "std::kv::open" => Value::Int(stdlib::kv_mod::open(&string!()).map_err(error)?),
        #[cfg(feature = "kv_mod")]
        "std::kv::close" => {
            stdlib::kv_mod::close(int!()).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "kv_mod")]
        "std::kv::flush" => {
            let bytes = stdlib::kv_mod::flush(int!()).map_err(error)?;
            Value::Int(
                i64::try_from(bytes)
                    .map_err(|_| "flushed byte count out of i64 range".to_string())?,
            )
        }
        #[cfg(feature = "kv_mod")]
        "std::kv::insert" => {
            let h = int!();
            let k = bytes!();
            let v = bytes!();
            stdlib::kv_mod::insert(h, &k, &v)
                .map_err(error)?
                .map(Value::Bytes)
                .unwrap_or(Value::Nil)
        }
        #[cfg(feature = "kv_mod")]
        "std::kv::get" => {
            let h = int!();
            let k = bytes!();
            stdlib::kv_mod::get(h, &k)
                .map_err(error)?
                .map(Value::Bytes)
                .unwrap_or(Value::Nil)
        }
        #[cfg(feature = "kv_mod")]
        "std::kv::remove" => {
            let h = int!();
            let k = bytes!();
            stdlib::kv_mod::remove(h, &k)
                .map_err(error)?
                .map(Value::Bytes)
                .unwrap_or(Value::Nil)
        }
        #[cfg(feature = "kv_mod")]
        "std::kv::contains" => {
            let h = int!();
            let k = bytes!();
            Value::Bool(stdlib::kv_mod::contains(h, &k).map_err(error)?)
        }
        #[cfg(feature = "kv_mod")]
        "std::kv::len" => Value::Int(stdlib::kv_mod::len(int!()).map_err(error)? as i64),
        #[cfg(feature = "kv_mod")]
        "std::kv::clear" => {
            stdlib::kv_mod::clear(int!()).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "kv_mod")]
        "std::kv::keys" => Value::Array(
            stdlib::kv_mod::keys(int!())
                .map_err(error)?
                .into_iter()
                .map(Value::Str)
                .collect(),
        ),
        #[cfg(feature = "kv_mod")]
        "std::kv::compare_and_swap" => {
            let h = int!();
            let k = bytes!();
            let e = bytes!();
            let n = bytes!();
            let expected = if e.is_empty() {
                None
            } else {
                Some(e.as_slice())
            };
            let new_value = if n.is_empty() {
                None
            } else {
                Some(n.as_slice())
            };
            Value::Bool(
                stdlib::kv_mod::compare_and_swap(h, &k, expected, new_value).map_err(error)?,
            )
        }
        #[cfg(feature = "kv_mod")]
        "std::kv::open_tree" => {
            let h = int!();
            let name = string!();
            Value::Int(stdlib::kv_mod::open_tree(h, &name).map_err(error)?)
        }
        #[cfg(feature = "kv_mod")]
        "std::kv::tree_insert" => {
            let h = int!();
            let k = bytes!();
            let v = bytes!();
            stdlib::kv_mod::tree_insert(h, &k, &v)
                .map_err(error)?
                .map(Value::Bytes)
                .unwrap_or(Value::Nil)
        }
        #[cfg(feature = "kv_mod")]
        "std::kv::tree_get" => {
            let h = int!();
            let k = bytes!();
            stdlib::kv_mod::tree_get(h, &k)
                .map_err(error)?
                .map(Value::Bytes)
                .unwrap_or(Value::Nil)
        }
        #[cfg(feature = "kv_mod")]
        "std::kv::tree_remove" => {
            let h = int!();
            let k = bytes!();
            stdlib::kv_mod::tree_remove(h, &k)
                .map_err(error)?
                .map(Value::Bytes)
                .unwrap_or(Value::Nil)
        }
        #[cfg(feature = "kv_mod")]
        "std::kv::tree_len" => Value::Int(stdlib::kv_mod::tree_len(int!()).map_err(error)? as i64),
        #[cfg(feature = "kv_mod")]
        "std::kv::tree_keys" => Value::Array(
            stdlib::kv_mod::tree_keys(int!())
                .map_err(error)?
                .into_iter()
                .map(Value::Str)
                .collect(),
        ),

        // ---------------- Phase 10: Redis client ----------------
        #[cfg(feature = "redis_mod")]
        "std::redis::connect" => Value::Int(stdlib::redis_mod::connect(&string!()).map_err(error)?),
        #[cfg(feature = "redis_mod")]
        "std::redis::close" => {
            stdlib::redis_mod::close(int!());
            Value::Nil
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::ping" => Value::Str(stdlib::redis_mod::ping(int!()).map_err(error)?),
        #[cfg(feature = "redis_mod")]
        "std::redis::set" => {
            let h = int!();
            let k = string!();
            let v = string!();
            stdlib::redis_mod::set(h, &k, &v).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::set_ex" => {
            let h = int!();
            let k = string!();
            let v = string!();
            let secs = u64::try_from(int!()).map_err(|_| "seconds out of range".to_string())?;
            stdlib::redis_mod::set_ex(h, &k, &v, secs).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::get" => {
            let h = int!();
            let k = string!();
            stdlib::redis_mod::get(h, &k)
                .map_err(error)?
                .map(Value::Str)
                .unwrap_or(Value::Nil)
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::del" => {
            let h = int!();
            let k = string!();
            Value::Int(stdlib::redis_mod::del(h, &k).map_err(error)? as i64)
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::exists" => {
            let h = int!();
            let k = string!();
            Value::Bool(stdlib::redis_mod::exists(h, &k).map_err(error)?)
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::expire" => {
            let h = int!();
            let k = string!();
            let s = int!();
            Value::Bool(stdlib::redis_mod::expire(h, &k, s).map_err(error)?)
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::ttl" => {
            let h = int!();
            let k = string!();
            Value::Int(stdlib::redis_mod::ttl(h, &k).map_err(error)?)
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::incr" => {
            let h = int!();
            let k = string!();
            let d = int!();
            Value::Int(stdlib::redis_mod::incr(h, &k, d).map_err(error)?)
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::keys" => {
            let h = int!();
            let pattern = string!();
            Value::Array(
                stdlib::redis_mod::keys(h, &pattern)
                    .map_err(error)?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::lpush" => {
            let h = int!();
            let k = string!();
            let v = string!();
            Value::Int(stdlib::redis_mod::lpush(h, &k, &v).map_err(error)? as i64)
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::rpush" => {
            let h = int!();
            let k = string!();
            let v = string!();
            Value::Int(stdlib::redis_mod::rpush(h, &k, &v).map_err(error)? as i64)
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::lrange" => {
            let h = int!();
            let k = string!();
            let start = int!();
            let stop = int!();
            Value::Array(
                stdlib::redis_mod::lrange(h, &k, start, stop)
                    .map_err(error)?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::llen" => {
            let h = int!();
            let k = string!();
            Value::Int(stdlib::redis_mod::llen(h, &k).map_err(error)? as i64)
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::hset" => {
            let h = int!();
            let k = string!();
            let f = string!();
            let v = string!();
            stdlib::redis_mod::hset(h, &k, &f, &v).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::hget" => {
            let h = int!();
            let k = string!();
            let f = string!();
            stdlib::redis_mod::hget(h, &k, &f)
                .map_err(error)?
                .map(Value::Str)
                .unwrap_or(Value::Nil)
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::hdel" => {
            let h = int!();
            let k = string!();
            let f = string!();
            Value::Int(stdlib::redis_mod::hdel(h, &k, &f).map_err(error)? as i64)
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::hgetall" => {
            let h = int!();
            let k = string!();
            let pairs = stdlib::redis_mod::hgetall(h, &k).map_err(error)?;
            Value::Array(
                pairs
                    .into_iter()
                    .map(|(k, v)| Value::Array(vec![Value::Str(k), Value::Str(v)]))
                    .collect(),
            )
        }
        #[cfg(feature = "redis_mod")]
        "std::redis::raw" => {
            let h = int!();
            let cmd = string!();
            Value::Str(stdlib::redis_mod::raw(h, &cmd).map_err(error)?)
        }

        // ---------------- Phase 11: HTTP server (tiny_http) ----------------
        #[cfg(feature = "server_mod")]
        "std::server::start" => Value::Int(stdlib::server_mod::start(&string!()).map_err(error)?),
        #[cfg(feature = "server_mod")]
        "std::server::local_addr" => {
            Value::Str(stdlib::server_mod::local_addr(int!()).map_err(error)?)
        }
        #[cfg(feature = "server_mod")]
        "std::server::accept" => {
            let h = int!();
            let to = u64::try_from(int!()).map_err(|_| "timeout must be nonnegative")?;
            Value::Int(stdlib::server_mod::accept(h, to).map_err(error)?)
        }
        #[cfg(feature = "server_mod")]
        "std::server::stop" => {
            stdlib::server_mod::stop(int!());
            Value::Nil
        }
        #[cfg(feature = "server_mod")]
        "std::server::method" => Value::Str(stdlib::server_mod::method(int!()).map_err(error)?),
        #[cfg(feature = "server_mod")]
        "std::server::url" => Value::Str(stdlib::server_mod::url(int!()).map_err(error)?),
        #[cfg(feature = "server_mod")]
        "std::server::path" => Value::Str(stdlib::server_mod::path(int!()).map_err(error)?),
        #[cfg(feature = "server_mod")]
        "std::server::query" => Value::Str(stdlib::server_mod::query(int!()).map_err(error)?),
        #[cfg(feature = "server_mod")]
        "std::server::remote_addr" => {
            Value::Str(stdlib::server_mod::remote_addr(int!()).map_err(error)?)
        }
        #[cfg(feature = "server_mod")]
        "std::server::header" => {
            let h = int!();
            let name = string!();
            match stdlib::server_mod::header(h, &name).map_err(error)? {
                Some(v) => Value::Str(v),
                None => Value::Nil,
            }
        }
        #[cfg(feature = "server_mod")]
        "std::server::headers" => {
            let h = int!();
            let hs = stdlib::server_mod::headers(h).map_err(error)?;
            Value::Map(hs.into_iter().map(|(k, v)| (k, Value::Str(v))).collect())
        }
        #[cfg(feature = "server_mod")]
        "std::server::body" => Value::Bytes(stdlib::server_mod::body(int!()).map_err(error)?),
        #[cfg(feature = "server_mod")]
        "std::server::body_text" => {
            Value::Str(stdlib::server_mod::body_text(int!()).map_err(error)?)
        }
        #[cfg(feature = "server_mod")]
        "std::server::respond" => {
            let h = int!();
            let status = u16::try_from(int!()).map_err(|_| "status out of range")?;
            let body = string!();
            stdlib::server_mod::respond(h, status, &body).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "server_mod")]
        "std::server::respond_html" => {
            let h = int!();
            let status = u16::try_from(int!()).map_err(|_| "status out of range")?;
            let body = string!();
            stdlib::server_mod::respond_html(h, status, &body).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "server_mod")]
        "std::server::respond_json" => {
            let h = int!();
            let status = u16::try_from(int!()).map_err(|_| "status out of range")?;
            let body = string!();
            stdlib::server_mod::respond_json(h, status, &body).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "server_mod")]
        "std::server::respond_bytes" => {
            let h = int!();
            let status = u16::try_from(int!()).map_err(|_| "status out of range")?;
            let ctype = string!();
            let data = bytes!();
            stdlib::server_mod::respond_bytes(h, status, &ctype, data).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "server_mod")]
        "std::server::respond_full" => {
            let h = int!();
            let status = u16::try_from(int!()).map_err(|_| "status out of range")?;
            let ctype = string!();
            let headers_map = expect_map(take!())?;
            let mut headers: Vec<(String, String)> = Vec::with_capacity(headers_map.len());
            for (k, v) in headers_map {
                headers.push((k, expect_string(v)?));
            }
            let data = bytes!();
            stdlib::server_mod::respond_full(h, status, &ctype, &headers, data).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "server_mod")]
        "std::server::upgrade_websocket" => {
            let h = int!();
            let max = nonnegative(int!())?;
            Value::Int(stdlib::server_mod::upgrade_websocket(h, max).map_err(error)?)
        }
        #[cfg(feature = "server_mod")]
        "std::server::ws_recv" => {
            let (kind, text, bytes) = stdlib::server_mod::ws_recv(int!()).map_err(error)?;
            Value::Array(vec![
                Value::Str(kind),
                Value::Str(text),
                Value::Bytes(bytes),
            ])
        }
        #[cfg(feature = "server_mod")]
        "std::server::ws_send_text" => {
            let h = int!();
            let text = string!();
            stdlib::server_mod::ws_send_text(h, &text).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "server_mod")]
        "std::server::ws_send_binary" => {
            let h = int!();
            let data = bytes!();
            stdlib::server_mod::ws_send_binary(h, &data).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "server_mod")]
        "std::server::ws_close" => {
            let h = int!();
            let code_raw = int!();
            let reason = string!();
            let code = if code_raw <= 0 {
                None
            } else {
                Some(u16::try_from(code_raw).map_err(|_| "close code out of range")?)
            };
            stdlib::server_mod::ws_close(h, code, &reason).map_err(error)?;
            Value::Nil
        }

        // ---------------- Phase 11: URL router (matchit) ----------------
        #[cfg(feature = "router_mod")]
        "std::router::new" => Value::Int(stdlib::router_mod::new().map_err(error)?),
        #[cfg(feature = "router_mod")]
        "std::router::drop" => {
            stdlib::router_mod::drop_router(int!());
            Value::Nil
        }
        #[cfg(feature = "router_mod")]
        "std::router::insert" => {
            let h = int!();
            let pat = string!();
            let val = string!();
            stdlib::router_mod::insert(h, &pat, &val).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "router_mod")]
        "std::router::at" => {
            let h = int!();
            let p = string!();
            match stdlib::router_mod::at(h, &p).map_err(error)? {
                Some((pattern, params)) => {
                    let mut m = BTreeMap::new();
                    m.insert("pattern".into(), Value::Str(pattern));
                    m.insert(
                        "params".into(),
                        Value::Map(
                            params
                                .into_iter()
                                .map(|(k, v)| (k, Value::Str(v)))
                                .collect(),
                        ),
                    );
                    Value::Map(m)
                }
                None => Value::Nil,
            }
        }
        #[cfg(feature = "router_mod")]
        "std::router::matches" => {
            let h = int!();
            let p = string!();
            Value::Bool(stdlib::router_mod::matches(h, &p).map_err(error)?)
        }

        // ---------------- Phase 14: charts (plotters, SVG) ----------------
        #[cfg(feature = "plot_mod")]
        "std::plot::line" => {
            let path = string!();
            let title = string!();
            let xa = string!();
            let ya = string!();
            let xs = array!()
                .into_iter()
                .map(expect_float)
                .collect::<Result<Vec<_>, _>>()?;
            let ys = array!()
                .into_iter()
                .map(expect_float)
                .collect::<Result<Vec<_>, _>>()?;
            stdlib::plot_mod::line_svg(&path, &title, &xa, &ya, &xs, &ys).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "plot_mod")]
        "std::plot::multi_line" => {
            let path = string!();
            let title = string!();
            let xa = string!();
            let ya = string!();
            // 3 parallel arrays: labels, xs-of-series, ys-of-series.
            let labels_arr = array!();
            let xss_arr = array!();
            let yss_arr = array!();
            if labels_arr.len() != xss_arr.len() || labels_arr.len() != yss_arr.len() {
                return Err(
                    "multi_line: labels, xs and ys arrays must all have the same length".into(),
                );
            }
            let mut series: Vec<(String, Vec<f64>, Vec<f64>)> =
                Vec::with_capacity(labels_arr.len());
            for ((label_v, xs_v), ys_v) in labels_arr
                .into_iter()
                .zip(xss_arr.into_iter())
                .zip(yss_arr.into_iter())
            {
                let label = expect_string(label_v)?;
                let xs = expect_array(xs_v)?
                    .into_iter()
                    .map(expect_float)
                    .collect::<Result<Vec<_>, _>>()?;
                let ys = expect_array(ys_v)?
                    .into_iter()
                    .map(expect_float)
                    .collect::<Result<Vec<_>, _>>()?;
                series.push((label, xs, ys));
            }
            stdlib::plot_mod::multi_line_svg(&path, &title, &xa, &ya, &series).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "plot_mod")]
        "std::plot::bar" => {
            let path = string!();
            let title = string!();
            let ya = string!();
            let labels = array!()
                .into_iter()
                .map(expect_string)
                .collect::<Result<Vec<_>, _>>()?;
            let values = array!()
                .into_iter()
                .map(expect_float)
                .collect::<Result<Vec<_>, _>>()?;
            stdlib::plot_mod::bar_svg(&path, &title, &ya, &labels, &values).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "plot_mod")]
        "std::plot::scatter" => {
            let path = string!();
            let title = string!();
            let xa = string!();
            let ya = string!();
            let xs = array!()
                .into_iter()
                .map(expect_float)
                .collect::<Result<Vec<_>, _>>()?;
            let ys = array!()
                .into_iter()
                .map(expect_float)
                .collect::<Result<Vec<_>, _>>()?;
            stdlib::plot_mod::scatter_svg(&path, &title, &xa, &ya, &xs, &ys).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "plot_mod")]
        "std::plot::histogram" => {
            let path = string!();
            let title = string!();
            let xa = string!();
            let values = array!()
                .into_iter()
                .map(expect_float)
                .collect::<Result<Vec<_>, _>>()?;
            let bins = nonnegative(int!())?;
            stdlib::plot_mod::histogram_svg(&path, &title, &xa, &values, bins).map_err(error)?;
            Value::Nil
        }

        // ---------------- Phase 12: HuggingFace tokenizers ----------------
        #[cfg(feature = "tokenize_mod")]
        "std::tokenize::load" => Value::Int(stdlib::tokenize_mod::load(&string!()).map_err(error)?),
        #[cfg(feature = "tokenize_mod")]
        "std::tokenize::from_json" => {
            Value::Int(stdlib::tokenize_mod::from_json(&string!()).map_err(error)?)
        }
        #[cfg(feature = "tokenize_mod")]
        "std::tokenize::close" => {
            stdlib::tokenize_mod::close(int!());
            Value::Nil
        }
        #[cfg(feature = "tokenize_mod")]
        "std::tokenize::vocab_size" => {
            Value::Int(stdlib::tokenize_mod::vocab_size(int!()).map_err(error)? as i64)
        }
        #[cfg(feature = "tokenize_mod")]
        "std::tokenize::encode" => {
            let h = int!();
            let text = string!();
            let special = boolean!();
            let enc = stdlib::tokenize_mod::encode(h, &text, special).map_err(error)?;
            Value::Map(encoding_to_map(enc))
        }
        #[cfg(feature = "tokenize_mod")]
        "std::tokenize::encode_padded" => {
            let h = int!();
            let text = string!();
            let max_length = nonnegative(int!())?;
            let pad_raw = int!();
            let pad_id =
                u32::try_from(pad_raw).map_err(|_| "pad_id out of u32 range".to_string())?;
            let special = boolean!();
            let enc = stdlib::tokenize_mod::encode_padded(h, &text, max_length, pad_id, special)
                .map_err(error)?;
            Value::Map(encoding_to_map(enc))
        }
        #[cfg(feature = "tokenize_mod")]
        "std::tokenize::encode_batch" => {
            let h = int!();
            let texts = array!()
                .into_iter()
                .map(expect_string)
                .collect::<Result<Vec<_>, _>>()?;
            let special = boolean!();
            let batch = stdlib::tokenize_mod::encode_batch(h, &texts, special).map_err(error)?;
            Value::Array(
                batch
                    .into_iter()
                    .map(|e| Value::Map(encoding_to_map(e)))
                    .collect(),
            )
        }
        #[cfg(feature = "tokenize_mod")]
        "std::tokenize::decode" => {
            let h = int!();
            let ids: Vec<u32> = array!()
                .into_iter()
                .map(|v| {
                    let n = expect_int(v)?;
                    u32::try_from(n).map_err(|_| "token id out of u32 range".to_string())
                })
                .collect::<Result<Vec<_>, String>>()?;
            let skip = boolean!();
            Value::Str(stdlib::tokenize_mod::decode(h, &ids, skip).map_err(error)?)
        }
        #[cfg(feature = "tokenize_mod")]
        "std::tokenize::token_to_id" => {
            let h = int!();
            let tok = string!();
            match stdlib::tokenize_mod::token_to_id(h, &tok).map_err(error)? {
                Some(id) => Value::Int(id as i64),
                None => Value::Nil,
            }
        }
        #[cfg(feature = "tokenize_mod")]
        "std::tokenize::id_to_token" => {
            let h = int!();
            let id_raw = int!();
            let id = u32::try_from(id_raw).map_err(|_| "token id out of u32 range".to_string())?;
            match stdlib::tokenize_mod::id_to_token(h, id).map_err(error)? {
                Some(s) => Value::Str(s),
                None => Value::Nil,
            }
        }

        // ---------------- Phase 12 part 2: ONNX inference (tract) ----------------
        #[cfg(feature = "onnx_mod")]
        "std::onnx::load" => Value::Int(stdlib::onnx_mod::load(&string!()).map_err(error)?),
        #[cfg(feature = "onnx_mod")]
        "std::onnx::load_shape" => {
            let path = string!();
            let shape_values = array!();
            stdlib::onnx_mod::preflight_shape_rank(shape_values.len()).map_err(error)?;
            let shape = shape_values
                .into_iter()
                .map(expect_int)
                .collect::<Result<Vec<i64>, _>>()?;
            Value::Int(stdlib::onnx_mod::load_with_input_shape(&path, &shape).map_err(error)?)
        }
        #[cfg(feature = "onnx_mod")]
        "std::onnx::close" => {
            stdlib::onnx_mod::close(int!());
            Value::Nil
        }
        #[cfg(feature = "onnx_mod")]
        "std::onnx::input_count" => {
            Value::Int(stdlib::onnx_mod::input_count(int!()).map_err(error)? as i64)
        }
        #[cfg(feature = "onnx_mod")]
        "std::onnx::output_count" => {
            Value::Int(stdlib::onnx_mod::output_count(int!()).map_err(error)? as i64)
        }
        #[cfg(feature = "onnx_mod")]
        "std::onnx::input_shape" => {
            let h = int!();
            let i = nonnegative(int!())?;
            let shape = stdlib::onnx_mod::input_shape(h, i).map_err(error)?;
            Value::Array(shape.into_iter().map(Value::Int).collect())
        }
        #[cfg(feature = "onnx_mod")]
        "std::onnx::output_shape" => {
            let h = int!();
            let i = nonnegative(int!())?;
            let shape = stdlib::onnx_mod::output_shape(h, i).map_err(error)?;
            Value::Array(shape.into_iter().map(Value::Int).collect())
        }
        #[cfg(feature = "onnx_mod")]
        "std::onnx::run_f32" => {
            let h = int!();
            let shape_values = array!();
            stdlib::onnx_mod::preflight_shape_rank(shape_values.len()).map_err(error)?;
            let shape = shape_values
                .into_iter()
                .map(expect_int)
                .collect::<Result<Vec<i64>, _>>()?;
            let data_values = array!();
            stdlib::onnx_mod::preflight_input_lengths(
                &shape,
                &[data_values.len()],
                std::mem::size_of::<f32>(),
            )
            .map_err(error)?;
            let data: Vec<f32> = data_values
                .into_iter()
                .map(|value| {
                    let value = expect_float(value)?;
                    if !value.is_finite() || value < f32::MIN as f64 || value > f32::MAX as f64 {
                        return Err("ONNX input float out of f32 range".to_string());
                    }
                    Ok(value as f32)
                })
                .collect::<Result<Vec<_>, String>>()?;
            let (values, out_shape) = stdlib::onnx_mod::run_f32(h, &shape, &data).map_err(error)?;
            Value::Map(onnx_output_to_map(values, out_shape))
        }
        #[cfg(feature = "onnx_mod")]
        "std::onnx::run_ids" => {
            let h = int!();
            let shape_values = array!();
            stdlib::onnx_mod::preflight_shape_rank(shape_values.len()).map_err(error)?;
            let shape = shape_values
                .into_iter()
                .map(expect_int)
                .collect::<Result<Vec<i64>, _>>()?;
            let data_values = array!();
            stdlib::onnx_mod::preflight_input_lengths(
                &shape,
                &[data_values.len()],
                std::mem::size_of::<i64>(),
            )
            .map_err(error)?;
            let data = data_values
                .into_iter()
                .map(expect_int)
                .collect::<Result<Vec<i64>, _>>()?;
            let (values, out_shape) =
                stdlib::onnx_mod::run_i64_in_f32_out(h, &shape, &data).map_err(error)?;
            Value::Map(onnx_output_to_map(values, out_shape))
        }
        #[cfg(feature = "onnx_mod")]
        "std::onnx::load_bert" => {
            let path = string!();
            let batch = int!();
            let seq = int!();
            Value::Int(stdlib::onnx_mod::load_bert_shape(&path, batch, seq).map_err(error)?)
        }
        #[cfg(feature = "onnx_mod")]
        "std::onnx::load_bert3" => {
            let path = string!();
            let batch = int!();
            let seq = int!();
            Value::Int(stdlib::onnx_mod::load_bert3_shape(&path, batch, seq).map_err(error)?)
        }
        #[cfg(feature = "onnx_mod")]
        "std::onnx::run_bert" => {
            let h = int!();
            let shape_values = array!();
            stdlib::onnx_mod::preflight_shape_rank(shape_values.len()).map_err(error)?;
            let shape = shape_values
                .into_iter()
                .map(expect_int)
                .collect::<Result<Vec<i64>, _>>()?;
            let id_values = array!();
            let mask_values = array!();
            stdlib::onnx_mod::preflight_input_lengths(
                &shape,
                &[id_values.len(), mask_values.len()],
                std::mem::size_of::<i64>(),
            )
            .map_err(error)?;
            let ids = id_values
                .into_iter()
                .map(expect_int)
                .collect::<Result<Vec<i64>, _>>()?;
            let mask = mask_values
                .into_iter()
                .map(expect_int)
                .collect::<Result<Vec<i64>, _>>()?;
            let (values, out_shape) =
                stdlib::onnx_mod::run_two_i64(h, &shape, &ids, &mask).map_err(error)?;
            Value::Map(onnx_output_to_map(values, out_shape))
        }
        #[cfg(feature = "onnx_mod")]
        "std::onnx::run_bert3" => {
            let h = int!();
            let shape_values = array!();
            stdlib::onnx_mod::preflight_shape_rank(shape_values.len()).map_err(error)?;
            let shape = shape_values
                .into_iter()
                .map(expect_int)
                .collect::<Result<Vec<i64>, _>>()?;
            let id_values = array!();
            let mask_values = array!();
            let type_values = array!();
            stdlib::onnx_mod::preflight_input_lengths(
                &shape,
                &[id_values.len(), mask_values.len(), type_values.len()],
                std::mem::size_of::<i64>(),
            )
            .map_err(error)?;
            let ids = id_values
                .into_iter()
                .map(expect_int)
                .collect::<Result<Vec<i64>, _>>()?;
            let mask = mask_values
                .into_iter()
                .map(expect_int)
                .collect::<Result<Vec<i64>, _>>()?;
            let types = type_values
                .into_iter()
                .map(expect_int)
                .collect::<Result<Vec<i64>, _>>()?;
            let (values, out_shape) =
                stdlib::onnx_mod::run_three_i64(h, &shape, &ids, &mask, &types).map_err(error)?;
            Value::Map(onnx_output_to_map(values, out_shape))
        }
        #[cfg(feature = "onnx_mod")]
        "std::onnx::run_bert_pooled" => {
            let h = int!();
            let batch = int!();
            let seq = int!();
            let id_values = array!();
            let mask_values = array!();
            stdlib::onnx_mod::preflight_input_lengths(
                &[batch, seq],
                &[id_values.len(), mask_values.len()],
                std::mem::size_of::<i64>(),
            )
            .map_err(error)?;
            let ids = id_values
                .into_iter()
                .map(expect_int)
                .collect::<Result<Vec<i64>, _>>()?;
            let mask = mask_values
                .into_iter()
                .map(expect_int)
                .collect::<Result<Vec<i64>, _>>()?;
            let (values, out_shape) =
                stdlib::onnx_mod::run_bert_pooled(h, batch, seq, &ids, &mask).map_err(error)?;
            Value::Map(onnx_output_to_map(values, out_shape))
        }

        // ---------------- Phase 12 pt.4: vector math ----------------
        #[cfg(feature = "vector_mod")]
        "std::vector::dot" => {
            let a: Vec<f32> = array!()
                .into_iter()
                .map(|v| Ok(expect_float(v)? as f32))
                .collect::<Result<Vec<_>, String>>()?;
            let b: Vec<f32> = array!()
                .into_iter()
                .map(|v| Ok(expect_float(v)? as f32))
                .collect::<Result<Vec<_>, String>>()?;
            if a.len() != b.len() {
                return Err(format!(
                    "std::vector::dot: length mismatch {} vs {}",
                    a.len(),
                    b.len()
                ));
            }
            Value::Float(stdlib::vector_mod::dot(&a, &b) as f64)
        }
        #[cfg(feature = "vector_mod")]
        "std::vector::norm" => {
            let v: Vec<f32> = array!()
                .into_iter()
                .map(|x| Ok(expect_float(x)? as f32))
                .collect::<Result<Vec<_>, String>>()?;
            Value::Float(stdlib::vector_mod::norm(&v) as f64)
        }
        #[cfg(feature = "vector_mod")]
        "std::vector::cosine_similarity" => {
            let a: Vec<f32> = array!()
                .into_iter()
                .map(|v| Ok(expect_float(v)? as f32))
                .collect::<Result<Vec<_>, String>>()?;
            let b: Vec<f32> = array!()
                .into_iter()
                .map(|v| Ok(expect_float(v)? as f32))
                .collect::<Result<Vec<_>, String>>()?;
            if a.len() != b.len() {
                return Err(format!(
                    "std::vector::cosine_similarity: length mismatch {} vs {}",
                    a.len(),
                    b.len()
                ));
            }
            Value::Float(stdlib::vector_mod::cosine_similarity(&a, &b) as f64)
        }
        #[cfg(feature = "vector_mod")]
        "std::vector::normalize" => {
            let v: Vec<f32> = array!()
                .into_iter()
                .map(|x| Ok(expect_float(x)? as f32))
                .collect::<Result<Vec<_>, String>>()?;
            let n = stdlib::vector_mod::normalize(&v);
            Value::Array(n.into_iter().map(|x| Value::Float(x as f64)).collect())
        }
        #[cfg(feature = "vector_mod")]
        "std::vector::add" => {
            let a: Vec<f32> = array!()
                .into_iter()
                .map(|v| Ok(expect_float(v)? as f32))
                .collect::<Result<Vec<_>, String>>()?;
            let b: Vec<f32> = array!()
                .into_iter()
                .map(|v| Ok(expect_float(v)? as f32))
                .collect::<Result<Vec<_>, String>>()?;
            if a.len() != b.len() {
                return Err(format!(
                    "std::vector::add: length mismatch {} vs {}",
                    a.len(),
                    b.len()
                ));
            }
            let r = stdlib::vector_mod::add(&a, &b);
            Value::Array(r.into_iter().map(|x| Value::Float(x as f64)).collect())
        }
        #[cfg(feature = "vector_mod")]
        "std::vector::sub" => {
            let a: Vec<f32> = array!()
                .into_iter()
                .map(|v| Ok(expect_float(v)? as f32))
                .collect::<Result<Vec<_>, String>>()?;
            let b: Vec<f32> = array!()
                .into_iter()
                .map(|v| Ok(expect_float(v)? as f32))
                .collect::<Result<Vec<_>, String>>()?;
            if a.len() != b.len() {
                return Err(format!(
                    "std::vector::sub: length mismatch {} vs {}",
                    a.len(),
                    b.len()
                ));
            }
            let r = stdlib::vector_mod::sub(&a, &b);
            Value::Array(r.into_iter().map(|x| Value::Float(x as f64)).collect())
        }
        #[cfg(feature = "vector_mod")]
        "std::vector::scale" => {
            let v: Vec<f32> = array!()
                .into_iter()
                .map(|x| Ok(expect_float(x)? as f32))
                .collect::<Result<Vec<_>, String>>()?;
            let k = float!() as f32;
            let r = stdlib::vector_mod::scale(&v, k);
            Value::Array(r.into_iter().map(|x| Value::Float(x as f64)).collect())
        }
        #[cfg(feature = "vector_mod")]
        "std::vector::argmax" => {
            let v: Vec<f32> = array!()
                .into_iter()
                .map(|x| Ok(expect_float(x)? as f32))
                .collect::<Result<Vec<_>, String>>()?;
            match stdlib::vector_mod::argmax(&v) {
                Some(i) => Value::Int(i as i64),
                None => return Err("std::vector::argmax: empty vector".into()),
            }
        }

        // ---------------- Phase 16: PDF generation (printpdf) ----------------
        #[cfg(feature = "pdf_mod")]
        "std::pdf::new" => {
            let title = string!();
            let w = float!();
            let h = float!();
            Value::Int(stdlib::pdf_mod::new(&title, w, h).map_err(error)?)
        }
        #[cfg(feature = "pdf_mod")]
        "std::pdf::add_page" => {
            let handle = int!();
            let w = float!();
            let h = float!();
            let name = string!();
            Value::Int(stdlib::pdf_mod::add_page(handle, w, h, &name).map_err(error)? as i64)
        }
        #[cfg(feature = "pdf_mod")]
        "std::pdf::page_count" => {
            Value::Int(stdlib::pdf_mod::page_count(int!()).map_err(error)? as i64)
        }
        #[cfg(feature = "pdf_mod")]
        "std::pdf::add_text" => {
            let handle = int!();
            let page = nonnegative(int!())?;
            let layer = nonnegative(int!())?;
            let text = string!();
            let size = float!();
            let x = float!();
            let y = float!();
            stdlib::pdf_mod::add_text(handle, page, layer, &text, size, x, y).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "pdf_mod")]
        "std::pdf::set_color" => {
            let handle = int!();
            let page = nonnegative(int!())?;
            let layer = nonnegative(int!())?;
            let r = float!();
            let g = float!();
            let b = float!();
            stdlib::pdf_mod::set_color(handle, page, layer, r, g, b).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "pdf_mod")]
        "std::pdf::add_line" => {
            let handle = int!();
            let page = nonnegative(int!())?;
            let layer = nonnegative(int!())?;
            let x1 = float!();
            let y1 = float!();
            let x2 = float!();
            let y2 = float!();
            let t = float!();
            stdlib::pdf_mod::add_line(handle, page, layer, x1, y1, x2, y2, t).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "pdf_mod")]
        "std::pdf::add_rect" => {
            let handle = int!();
            let page = nonnegative(int!())?;
            let layer = nonnegative(int!())?;
            let x = float!();
            let y = float!();
            let w = float!();
            let h = float!();
            stdlib::pdf_mod::add_rect(handle, page, layer, x, y, w, h).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "pdf_mod")]
        "std::pdf::save" => {
            let handle = int!();
            let path = string!();
            stdlib::pdf_mod::save(handle, &path).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "pdf_mod")]
        "std::pdf::close" => {
            stdlib::pdf_mod::close(int!());
            Value::Nil
        }

        // ---------------- Phase 13': Wi-Fi introspection ----------------
        #[cfg(feature = "wifi_mod")]
        "std::wifi::scan" => {
            let aps = stdlib::wifi_mod::scan().map_err(error)?;
            Value::Array(
                aps.into_iter()
                    .map(|ap| {
                        Value::Map(BTreeMap::from([
                            ("ssid".into(), Value::Str(ap.ssid)),
                            ("bssid".into(), Value::Str(ap.bssid)),
                            ("rssi".into(), Value::Int(ap.rssi)),
                            ("frequency_mhz".into(), Value::Int(ap.frequency_mhz)),
                            ("timestamp".into(), Value::Int(ap.timestamp)),
                            (
                                "channel_bandwidth_mhz".into(),
                                Value::Str(ap.channel_bandwidth_mhz),
                            ),
                            (
                                "center_frequency_mhz".into(),
                                Value::Int(ap.center_frequency_mhz),
                            ),
                        ]))
                    })
                    .collect(),
            )
        }
        #[cfg(feature = "wifi_mod")]
        "std::wifi::connection_info" => {
            match stdlib::wifi_mod::connection_info().map_err(error)? {
                Some(ci) => Value::Map(BTreeMap::from([
                    ("ssid".into(), Value::Str(ci.ssid)),
                    ("bssid".into(), Value::Str(ci.bssid)),
                    ("ip".into(), Value::Str(ci.ip)),
                    ("mac_address".into(), Value::Str(ci.mac_address)),
                    ("link_speed_mbps".into(), Value::Int(ci.link_speed_mbps)),
                    ("rssi".into(), Value::Int(ci.rssi)),
                    ("frequency_mhz".into(), Value::Int(ci.frequency_mhz)),
                    ("network_id".into(), Value::Int(ci.network_id)),
                    ("supplicant_state".into(), Value::Str(ci.supplicant_state)),
                    ("hidden_ssid".into(), Value::Bool(ci.hidden_ssid)),
                ])),
                None => Value::Nil,
            }
        }
        #[cfg(feature = "wifi_mod")]
        "std::wifi::set_enabled" => {
            stdlib::wifi_mod::set_enabled(boolean!()).map_err(error)?;
            Value::Nil
        }
        #[cfg(feature = "wifi_mod")]
        "std::wifi::signal_bars" => Value::Int(stdlib::wifi_mod::signal_bars(int!()) as i64),

        // ---------------- Phase 1: dirs ----------------
        #[cfg(feature = "dirs_mod")]
        "std::dirs::home" => Value::Str(stdlib::dirs_mod::home()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::config" => Value::Str(stdlib::dirs_mod::config()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::cache" => Value::Str(stdlib::dirs_mod::cache()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::data" => Value::Str(stdlib::dirs_mod::data()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::data_local" => Value::Str(stdlib::dirs_mod::data_local()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::state" => Value::Str(stdlib::dirs_mod::state()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::executable" => Value::Str(stdlib::dirs_mod::executable()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::runtime" => Value::Str(stdlib::dirs_mod::runtime()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::preference" => Value::Str(stdlib::dirs_mod::preference()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::desktop" => Value::Str(stdlib::dirs_mod::desktop()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::documents" => Value::Str(stdlib::dirs_mod::documents()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::downloads" => Value::Str(stdlib::dirs_mod::downloads()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::pictures" => Value::Str(stdlib::dirs_mod::pictures()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::music" => Value::Str(stdlib::dirs_mod::music()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::videos" => Value::Str(stdlib::dirs_mod::videos()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::public" => Value::Str(stdlib::dirs_mod::public()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::temp" => Value::Str(stdlib::dirs_mod::temp()),
        #[cfg(feature = "dirs_mod")]
        "std::dirs::current" => Value::Str(stdlib::dirs_mod::current()),

        // ---------------- Phase 34: std::process ----------------
        #[cfg(feature = "process_mod")]
        "std::process::run" => {
            let out = stdlib::process_mod::run(&string!()).map_err(|e| e.to_string())?;
            process_output_to_value(out)
        }
        #[cfg(feature = "process_mod")]
        "std::process::run_with_input" => {
            let cmd = string!();
            let input = bytes!();
            let out =
                stdlib::process_mod::run_with_input(&cmd, &input).map_err(|e| e.to_string())?;
            process_output_to_value(out)
        }
        #[cfg(feature = "process_mod")]
        "std::process::shell" => {
            let out = stdlib::process_mod::shell(&string!()).map_err(|e| e.to_string())?;
            process_output_to_value(out)
        }
        #[cfg(feature = "process_mod")]
        "std::process::pipe" => {
            let arr = array!();
            let cmds: Vec<String> = arr
                .into_iter()
                .map(expect_string)
                .collect::<Result<_, _>>()?;
            let out = stdlib::process_mod::pipe(&cmds).map_err(|e| e.to_string())?;
            process_output_to_value(out)
        }
        #[cfg(feature = "process_mod")]
        "std::process::spawn" => {
            let h = stdlib::process_mod::spawn_bg(&string!()).map_err(|e| e.to_string())?;
            Value::Int(h as i64)
        }
        #[cfg(feature = "process_mod")]
        "std::process::spawn_wait" => {
            let h = u64::try_from(int!())
                .map_err(|_| "spawn_wait handle must be nonneg".to_string())?;
            let out = stdlib::process_mod::spawn_wait(h).map_err(|e| e.to_string())?;
            process_output_to_value(out)
        }
        #[cfg(feature = "process_mod")]
        "std::process::spawn_poll" => {
            let h = u64::try_from(int!())
                .map_err(|_| "spawn_poll handle must be nonneg".to_string())?;
            match stdlib::process_mod::spawn_poll(h).map_err(|e| e.to_string())? {
                Some(code) => Value::Enum {
                    name: "Option".into(),
                    variant: "Some".into(),
                    payload: Some(Box::new(Value::Int(code as i64))),
                },
                None => Value::Enum {
                    name: "Option".into(),
                    variant: "None".into(),
                    payload: None,
                },
            }
        }
        #[cfg(feature = "process_mod")]
        "std::process::spawn_kill" => {
            let h = u64::try_from(int!())
                .map_err(|_| "spawn_kill handle must be nonneg".to_string())?;
            stdlib::process_mod::spawn_kill(h).map_err(|e| e.to_string())?;
            Value::Nil
        }
        #[cfg(feature = "process_mod")]
        "std::process::spawn_pid" => {
            let h =
                u64::try_from(int!()).map_err(|_| "spawn_pid handle must be nonneg".to_string())?;
            let pid = stdlib::process_mod::spawn_pid(h).map_err(|e| e.to_string())?;
            Value::Int(pid as i64)
        }
        #[cfg(feature = "process_mod")]
        "std::process::env_get" => match stdlib::process_mod::env_get(&string!()) {
            Some(v) => Value::Enum {
                name: "Option".into(),
                variant: "Some".into(),
                payload: Some(Box::new(Value::Str(v))),
            },
            None => Value::Enum {
                name: "Option".into(),
                variant: "None".into(),
                payload: None,
            },
        },
        #[cfg(feature = "process_mod")]
        "std::process::env_set" => {
            let name = string!();
            let value = string!();
            stdlib::process_mod::env_set(&name, &value);
            Value::Nil
        }
        #[cfg(feature = "process_mod")]
        "std::process::env_unset" => {
            stdlib::process_mod::env_unset(&string!());
            Value::Nil
        }
        #[cfg(feature = "process_mod")]
        "std::process::env_vars" => Value::Array(
            stdlib::process_mod::env_vars()
                .into_iter()
                .map(|(k, v)| Value::Tuple(vec![Value::Str(k), Value::Str(v)]))
                .collect(),
        ),
        #[cfg(feature = "process_mod")]
        "std::process::working_dir" => {
            Value::Str(stdlib::process_mod::working_dir().map_err(|e| e.to_string())?)
        }
        #[cfg(feature = "process_mod")]
        "std::process::set_working_dir" => {
            stdlib::process_mod::set_working_dir(&string!()).map_err(|e| e.to_string())?;
            Value::Nil
        }
        #[cfg(feature = "process_mod")]
        "std::process::self_pid" => Value::Int(stdlib::process_mod::self_pid() as i64),
        #[cfg(feature = "process_mod")]
        "std::process::hostname" => Value::Str(stdlib::process_mod::hostname()),
        #[cfg(feature = "process_mod")]
        "std::process::username" => Value::Str(stdlib::process_mod::username()),
        #[cfg(feature = "process_mod")]
        "std::process::args" => Value::Array(
            stdlib::process_mod::args()
                .into_iter()
                .map(Value::Str)
                .collect(),
        ),
        #[cfg(feature = "process_mod")]
        "std::process::send_signal" => {
            let pid = int!() as i32;
            let sig = int!() as i32;
            stdlib::process_mod::send_signal(pid, sig).map_err(|e| e.to_string())?;
            Value::Nil
        }
        #[cfg(feature = "process_mod")]
        "std::process::exit" => {
            stdlib::process_mod::exit(int!() as i32);
        }

        // ---------------- Phase 34: std::collections ----------------
        #[cfg(feature = "collections_mod")]
        "std::collections::set_new" => Value::Int(stdlib::collections_mod::set_new()? as i64),
        #[cfg(feature = "collections_mod")]
        "std::collections::set_from" => {
            let arr = array!();
            let items: Vec<String> = arr
                .into_iter()
                .map(expect_string)
                .collect::<Result<_, _>>()?;
            Value::Int(stdlib::collections_mod::set_from(items)? as i64)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::set_add" => {
            let h = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            Value::Bool(stdlib::collections_mod::set_add(h, string!())?)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::set_remove" => {
            let h = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            Value::Bool(stdlib::collections_mod::set_remove(h, &string!())?)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::set_contains" => {
            let h = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            Value::Bool(stdlib::collections_mod::set_contains(h, &string!())?)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::set_len" => {
            let h = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            Value::Int(stdlib::collections_mod::set_len(h)? as i64)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::set_to_array" => {
            let h = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            Value::Array(
                stdlib::collections_mod::set_to_array(h)?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::set_union" => {
            let a = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            let b = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            Value::Int(stdlib::collections_mod::set_union(a, b)? as i64)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::set_intersect" => {
            let a = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            let b = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            Value::Int(stdlib::collections_mod::set_intersect(a, b)? as i64)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::set_difference" => {
            let a = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            let b = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            Value::Int(stdlib::collections_mod::set_difference(a, b)? as i64)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::set_is_subset" => {
            let a = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            let b = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            Value::Bool(stdlib::collections_mod::set_is_subset(a, b)?)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::set_drop" => {
            let h = u64::try_from(int!()).map_err(|_| "set handle must be nonneg".to_string())?;
            Value::Bool(stdlib::collections_mod::set_drop(h))
        }
        // Deque
        #[cfg(feature = "collections_mod")]
        "std::collections::deque_new" => Value::Int(stdlib::collections_mod::deque_new()? as i64),
        #[cfg(feature = "collections_mod")]
        "std::collections::deque_push_front" => {
            let h = u64::try_from(int!()).map_err(|_| "deque handle".to_string())?;
            stdlib::collections_mod::deque_push_front(h, string!())?;
            Value::Nil
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::deque_push_back" => {
            let h = u64::try_from(int!()).map_err(|_| "deque handle".to_string())?;
            stdlib::collections_mod::deque_push_back(h, string!())?;
            Value::Nil
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::deque_pop_front" => {
            let h = u64::try_from(int!()).map_err(|_| "deque handle".to_string())?;
            match stdlib::collections_mod::deque_pop_front(h)? {
                Some(v) => Value::Enum {
                    name: "Option".into(),
                    variant: "Some".into(),
                    payload: Some(Box::new(Value::Str(v))),
                },
                None => Value::Enum {
                    name: "Option".into(),
                    variant: "None".into(),
                    payload: None,
                },
            }
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::deque_pop_back" => {
            let h = u64::try_from(int!()).map_err(|_| "deque handle".to_string())?;
            match stdlib::collections_mod::deque_pop_back(h)? {
                Some(v) => Value::Enum {
                    name: "Option".into(),
                    variant: "Some".into(),
                    payload: Some(Box::new(Value::Str(v))),
                },
                None => Value::Enum {
                    name: "Option".into(),
                    variant: "None".into(),
                    payload: None,
                },
            }
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::deque_len" => {
            let h = u64::try_from(int!()).map_err(|_| "deque handle".to_string())?;
            Value::Int(stdlib::collections_mod::deque_len(h)? as i64)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::deque_to_array" => {
            let h = u64::try_from(int!()).map_err(|_| "deque handle".to_string())?;
            Value::Array(
                stdlib::collections_mod::deque_to_array(h)?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::deque_drop" => {
            let h = u64::try_from(int!()).map_err(|_| "deque handle".to_string())?;
            Value::Bool(stdlib::collections_mod::deque_drop(h))
        }
        // PriorityQueue
        #[cfg(feature = "collections_mod")]
        "std::collections::pq_new_max" => Value::Int(stdlib::collections_mod::pq_new_max()? as i64),
        #[cfg(feature = "collections_mod")]
        "std::collections::pq_new_min" => Value::Int(stdlib::collections_mod::pq_new_min()? as i64),
        #[cfg(feature = "collections_mod")]
        "std::collections::pq_push" => {
            let h = u64::try_from(int!()).map_err(|_| "pq handle".to_string())?;
            let item = string!();
            let pri = int!();
            stdlib::collections_mod::pq_push(h, item, pri)?;
            Value::Nil
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::pq_pop" => {
            let h = u64::try_from(int!()).map_err(|_| "pq handle".to_string())?;
            match stdlib::collections_mod::pq_pop(h)? {
                Some(v) => Value::Enum {
                    name: "Option".into(),
                    variant: "Some".into(),
                    payload: Some(Box::new(Value::Str(v))),
                },
                None => Value::Enum {
                    name: "Option".into(),
                    variant: "None".into(),
                    payload: None,
                },
            }
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::pq_peek" => {
            let h = u64::try_from(int!()).map_err(|_| "pq handle".to_string())?;
            match stdlib::collections_mod::pq_peek(h)? {
                Some(v) => Value::Enum {
                    name: "Option".into(),
                    variant: "Some".into(),
                    payload: Some(Box::new(Value::Str(v))),
                },
                None => Value::Enum {
                    name: "Option".into(),
                    variant: "None".into(),
                    payload: None,
                },
            }
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::pq_len" => {
            let h = u64::try_from(int!()).map_err(|_| "pq handle".to_string())?;
            Value::Int(stdlib::collections_mod::pq_len(h)? as i64)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::pq_drop" => {
            let h = u64::try_from(int!()).map_err(|_| "pq handle".to_string())?;
            Value::Bool(stdlib::collections_mod::pq_drop(h))
        }
        // OrderedMap
        #[cfg(feature = "collections_mod")]
        "std::collections::omap_new" => Value::Int(stdlib::collections_mod::omap_new()? as i64),
        #[cfg(feature = "collections_mod")]
        "std::collections::omap_insert" => {
            let h = u64::try_from(int!()).map_err(|_| "omap handle".to_string())?;
            let k = string!();
            let v = to_json(take!())?;
            stdlib::collections_mod::omap_insert(h, k, v)?;
            Value::Nil
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::omap_get" => {
            let h = u64::try_from(int!()).map_err(|_| "omap handle".to_string())?;
            match stdlib::collections_mod::omap_get(h, &string!())? {
                Some(v) => Value::Enum {
                    name: "Option".into(),
                    variant: "Some".into(),
                    payload: Some(Box::new(from_json(v)?)),
                },
                None => Value::Enum {
                    name: "Option".into(),
                    variant: "None".into(),
                    payload: None,
                },
            }
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::omap_remove" => {
            let h = u64::try_from(int!()).map_err(|_| "omap handle".to_string())?;
            Value::Bool(stdlib::collections_mod::omap_remove(h, &string!())?)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::omap_keys" => {
            let h = u64::try_from(int!()).map_err(|_| "omap handle".to_string())?;
            Value::Array(
                stdlib::collections_mod::omap_keys(h)?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::omap_len" => {
            let h = u64::try_from(int!()).map_err(|_| "omap handle".to_string())?;
            Value::Int(stdlib::collections_mod::omap_len(h)? as i64)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::omap_drop" => {
            let h = u64::try_from(int!()).map_err(|_| "omap handle".to_string())?;
            Value::Bool(stdlib::collections_mod::omap_drop(h))
        }
        // Counter
        #[cfg(feature = "collections_mod")]
        "std::collections::counter_from" => {
            let arr = array!();
            let items: Vec<String> = arr
                .into_iter()
                .map(expect_string)
                .collect::<Result<_, _>>()?;
            Value::Int(stdlib::collections_mod::counter_from(items)? as i64)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::counter_add" => {
            let h = u64::try_from(int!()).map_err(|_| "counter handle".to_string())?;
            let item = string!();
            let delta = int!();
            stdlib::collections_mod::counter_add(h, item, delta)?;
            Value::Nil
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::counter_count" => {
            let h = u64::try_from(int!()).map_err(|_| "counter handle".to_string())?;
            Value::Int(stdlib::collections_mod::counter_count(h, &string!())?)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::counter_most_common" => {
            let h = u64::try_from(int!()).map_err(|_| "counter handle".to_string())?;
            let n = int!() as usize;
            let items = stdlib::collections_mod::counter_most_common(h, n)?;
            Value::Array(
                items
                    .into_iter()
                    .map(|(k, v)| Value::Tuple(vec![Value::Str(k), Value::Int(v)]))
                    .collect(),
            )
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::counter_total" => {
            let h = u64::try_from(int!()).map_err(|_| "counter handle".to_string())?;
            Value::Int(stdlib::collections_mod::counter_total(h)?)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::counter_drop" => {
            let h = u64::try_from(int!()).map_err(|_| "counter handle".to_string())?;
            Value::Bool(stdlib::collections_mod::counter_drop(h))
        }
        // Graph
        #[cfg(feature = "collections_mod")]
        "std::collections::graph_new" => {
            Value::Int(stdlib::collections_mod::graph_new(boolean!())? as i64)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::graph_add_node" => {
            let h = u64::try_from(int!()).map_err(|_| "graph handle".to_string())?;
            stdlib::collections_mod::graph_add_node(h, string!())?;
            Value::Nil
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::graph_add_edge" => {
            let h = u64::try_from(int!()).map_err(|_| "graph handle".to_string())?;
            let from = string!();
            let to = string!();
            let weight = int!();
            stdlib::collections_mod::graph_add_edge(h, from, to, weight)?;
            Value::Nil
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::graph_neighbors" => {
            let h = u64::try_from(int!()).map_err(|_| "graph handle".to_string())?;
            Value::Array(
                stdlib::collections_mod::graph_neighbors(h, &string!())?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::graph_bfs" => {
            let h = u64::try_from(int!()).map_err(|_| "graph handle".to_string())?;
            Value::Array(
                stdlib::collections_mod::graph_bfs(h, &string!())?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::graph_dfs" => {
            let h = u64::try_from(int!()).map_err(|_| "graph handle".to_string())?;
            Value::Array(
                stdlib::collections_mod::graph_dfs(h, &string!())?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::graph_shortest_path" => {
            let h = u64::try_from(int!()).map_err(|_| "graph handle".to_string())?;
            let start = string!();
            let end = string!();
            Value::Array(
                stdlib::collections_mod::graph_shortest_path(h, &start, &end)?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::graph_topological_sort" => {
            let h = u64::try_from(int!()).map_err(|_| "graph handle".to_string())?;
            Value::Array(
                stdlib::collections_mod::graph_topological_sort(h)?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::graph_has_cycle" => {
            let h = u64::try_from(int!()).map_err(|_| "graph handle".to_string())?;
            Value::Bool(stdlib::collections_mod::graph_has_cycle(h)?)
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::graph_nodes" => {
            let h = u64::try_from(int!()).map_err(|_| "graph handle".to_string())?;
            Value::Array(
                stdlib::collections_mod::graph_nodes(h)?
                    .into_iter()
                    .map(Value::Str)
                    .collect(),
            )
        }
        #[cfg(feature = "collections_mod")]
        "std::collections::graph_drop" => {
            let h = u64::try_from(int!()).map_err(|_| "graph handle".to_string())?;
            Value::Bool(stdlib::collections_mod::graph_drop(h))
        }

        // ---------------- Phase 34: std::datetime extendido ----------------
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::year" => Value::Int(stdlib::datetime_ext_mod::year(int!())),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::month" => Value::Int(stdlib::datetime_ext_mod::month(int!())),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::day" => Value::Int(stdlib::datetime_ext_mod::day(int!())),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::hour" => Value::Int(stdlib::datetime_ext_mod::hour(int!())),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::minute" => Value::Int(stdlib::datetime_ext_mod::minute(int!())),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::second" => Value::Int(stdlib::datetime_ext_mod::second(int!())),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::day_of_week" => Value::Int(stdlib::datetime_ext_mod::day_of_week(int!())),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::day_of_year" => Value::Int(stdlib::datetime_ext_mod::day_of_year(int!())),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::week_of_year" => Value::Int(stdlib::datetime_ext_mod::week_of_year(int!())),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::quarter" => Value::Int(stdlib::datetime_ext_mod::quarter(int!())),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::is_leap_year" => {
            Value::Bool(stdlib::datetime_ext_mod::is_leap_year(int!()))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::days_in_month" => {
            let y = int!();
            let m = int!();
            Value::Int(stdlib::datetime_ext_mod::days_in_month(y, m))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::add_seconds" => {
            let ts = int!();
            let n = int!();
            Value::Int(stdlib::datetime_ext_mod::add_seconds(ts, n))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::add_minutes" => {
            let ts = int!();
            let n = int!();
            Value::Int(stdlib::datetime_ext_mod::add_minutes(ts, n))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::add_hours" => {
            let ts = int!();
            let n = int!();
            Value::Int(stdlib::datetime_ext_mod::add_hours(ts, n))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::add_days_ext" => {
            let ts = int!();
            let n = int!();
            Value::Int(stdlib::datetime_ext_mod::add_days(ts, n))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::add_weeks" => {
            let ts = int!();
            let n = int!();
            Value::Int(stdlib::datetime_ext_mod::add_weeks(ts, n))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::add_months" => {
            let ts = int!();
            let n = int!();
            Value::Int(stdlib::datetime_ext_mod::add_months(ts, n))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::add_years" => {
            let ts = int!();
            let n = int!();
            Value::Int(stdlib::datetime_ext_mod::add_years(ts, n))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::diff_seconds" => {
            let a = int!();
            let b = int!();
            Value::Int(stdlib::datetime_ext_mod::diff_seconds(a, b))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::diff_minutes" => {
            let a = int!();
            let b = int!();
            Value::Int(stdlib::datetime_ext_mod::diff_minutes(a, b))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::diff_hours" => {
            let a = int!();
            let b = int!();
            Value::Int(stdlib::datetime_ext_mod::diff_hours(a, b))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::diff_days" => {
            let a = int!();
            let b = int!();
            Value::Int(stdlib::datetime_ext_mod::diff_days(a, b))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::is_before" => {
            let a = int!();
            let b = int!();
            Value::Bool(stdlib::datetime_ext_mod::is_before(a, b))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::is_after" => {
            let a = int!();
            let b = int!();
            Value::Bool(stdlib::datetime_ext_mod::is_after(a, b))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::is_same_day" => {
            let a = int!();
            let b = int!();
            Value::Bool(stdlib::datetime_ext_mod::is_same_day(a, b))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::to_timezone" => {
            let ts = int!();
            let tz = string!();
            Value::Str(stdlib::datetime_ext_mod::to_timezone(ts, &tz)?)
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::timezone_offset_seconds" => {
            let ts = int!();
            let tz = string!();
            Value::Int(stdlib::datetime_ext_mod::timezone_offset_seconds(ts, &tz)?)
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::common_timezones" => Value::Array(
            stdlib::datetime_ext_mod::common_timezones()
                .into_iter()
                .map(Value::Str)
                .collect(),
        ),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::to_iso" => Value::Str(stdlib::datetime_ext_mod::to_iso(int!())),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::from_iso" => Value::Int(stdlib::datetime_ext_mod::from_iso(&string!())?),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::humanize" => {
            let ts = int!();
            let now = int!();
            Value::Str(stdlib::datetime_ext_mod::humanize(ts, now))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::is_weekend" => Value::Bool(stdlib::datetime_ext_mod::is_weekend(int!())),
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::business_days_between" => {
            let a = int!();
            let b = int!();
            Value::Int(stdlib::datetime_ext_mod::business_days_between(a, b))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::next_weekday" => {
            let ts = int!();
            let dow = int!();
            Value::Int(stdlib::datetime_ext_mod::next_weekday(ts, dow))
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::from_ymd" => {
            let y = int!();
            let m = int!();
            let d = int!();
            Value::Int(stdlib::datetime_ext_mod::from_ymd(y, m, d)?)
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::from_ymd_hms" => {
            let y = int!();
            let m = int!();
            let d = int!();
            let h = int!();
            let mi = int!();
            let s = int!();
            Value::Int(stdlib::datetime_ext_mod::from_ymd_hms(y, m, d, h, mi, s)?)
        }
        #[cfg(feature = "datetime_ext_mod")]
        "std::datetime::range_ext" => {
            let start = int!();
            let end = int!();
            let step = int!();
            Value::Array(
                stdlib::datetime_ext_mod::range(start, end, step)
                    .into_iter()
                    .map(Value::Int)
                    .collect(),
            )
        }

        _ => return Err("registered function has no VM implementation".into()),
    })
}

/// Helper: convierte un ProcessOutput a un Titan Value::Map con las 4 keys.
#[cfg(feature = "process_mod")]
fn process_output_to_value(out: stdlib::process_mod::ProcessOutput) -> Value {
    let mut m = BTreeMap::new();
    m.insert("stdout".into(), Value::Str(out.stdout));
    m.insert("stderr".into(), Value::Str(out.stderr));
    m.insert("exit_code".into(), Value::Int(out.exit_code as i64));
    m.insert("duration_ms".into(), Value::Int(out.duration_ms as i64));
    Value::Map(m)
}

fn metrics_snapshot(snapshot: stdlib::metrics::Snapshot) -> Value {
    let counters = snapshot
        .counters
        .into_iter()
        .map(|(name, value)| (name, Value::Int(i64::try_from(value).unwrap_or(i64::MAX))))
        .collect();
    let gauges = snapshot
        .gauges
        .into_iter()
        .map(|(name, value)| (name, Value::Float(value)))
        .collect();
    let histograms = snapshot
        .histograms
        .into_iter()
        .map(|(name, value)| {
            (
                name,
                Value::Map(BTreeMap::from([
                    (
                        "count".into(),
                        Value::Int(i64::try_from(value.count).unwrap_or(i64::MAX)),
                    ),
                    ("sum".into(), Value::Float(value.sum)),
                    ("min".into(), Value::Float(value.min)),
                    ("max".into(), Value::Float(value.max)),
                ])),
            )
        })
        .collect();
    Value::Map(BTreeMap::from([
        ("counters".into(), Value::Map(counters)),
        ("gauges".into(), Value::Map(gauges)),
        ("histograms".into(), Value::Map(histograms)),
    ]))
}

#[cfg(feature = "onnx_mod")]
fn onnx_output_to_map(values: Vec<f32>, shape: Vec<usize>) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "values".into(),
            Value::Array(values.into_iter().map(|v| Value::Float(v as f64)).collect()),
        ),
        (
            "shape".into(),
            Value::Array(shape.into_iter().map(|d| Value::Int(d as i64)).collect()),
        ),
    ])
}

#[cfg(feature = "tokenize_mod")]
fn encoding_to_map(e: stdlib::tokenize_mod::Encoding) -> BTreeMap<String, Value> {
    let to_int_array =
        |xs: Vec<u32>| Value::Array(xs.into_iter().map(|n| Value::Int(n as i64)).collect());
    BTreeMap::from([
        ("ids".into(), to_int_array(e.ids)),
        (
            "tokens".into(),
            Value::Array(e.tokens.into_iter().map(Value::Str).collect()),
        ),
        ("type_ids".into(), to_int_array(e.type_ids)),
        ("attention_mask".into(), to_int_array(e.attention_mask)),
        (
            "special_tokens_mask".into(),
            to_int_array(e.special_tokens_mask),
        ),
    ])
}
fn websocket_upgrade(request: Value, protocol: &str) -> Result<Vec<u8>, String> {
    let Value::Map(request) = request else {
        return Err("WebSocket upgrade request must be map".into());
    };
    let method = match request.get("method") {
        Some(Value::Str(value)) => value,
        _ => return Err("WebSocket upgrade requires method".into()),
    };
    let version = match request.get("version") {
        Some(Value::Str(value)) => value,
        _ => return Err("WebSocket upgrade requires version".into()),
    };
    let headers = match request.get("headers") {
        Some(Value::Map(value)) => value,
        _ => return Err("WebSocket upgrade requires headers".into()),
    };
    if method != "GET" || version != "HTTP/1.1" {
        return Err("WebSocket upgrade requires GET HTTP/1.1".into());
    }
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|value| {
                if let Value::Str(value) = value {
                    Some(value.as_str())
                } else {
                    None
                }
            })
            .ok_or_else(|| format!("missing WebSocket header {name}"))
    };
    let upgrade = header("upgrade")?;
    let connection = header("connection")?;
    let ws_version = header("sec-websocket-version")?;
    let key = header("sec-websocket-key")?;
    if !upgrade.eq_ignore_ascii_case("websocket")
        || !connection
            .split(',')
            .any(|value| value.trim().eq_ignore_ascii_case("upgrade"))
        || ws_version != "13"
    {
        return Err("invalid WebSocket upgrade headers".into());
    }
    if !protocol.is_empty() {
        let offered = header("sec-websocket-protocol")?;
        if !offered.split(',').any(|value| value.trim() == protocol) {
            return Err("selected WebSocket protocol was not offered".into());
        }
    }
    stdlib::websocket::upgrade_response(
        key,
        if protocol.is_empty() {
            None
        } else {
            Some(protocol)
        },
    )
    .map_err(error)
}
fn websocket_validate_accept(response: &[u8], key: &str) -> Result<bool, String> {
    let Some(end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Ok(false);
    };
    let text = std::str::from_utf8(&response[..end]).map_err(error)?;
    let mut lines = text.split("\r\n");
    if lines.next() != Some("HTTP/1.1 101 Switching Protocols") {
        return Ok(false);
    }
    let mut headers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return Ok(false);
        };
        headers
            .entry(name.to_ascii_lowercase())
            .or_default()
            .push(value.trim().into());
    }
    let single = |name: &str| {
        headers
            .get(name)
            .filter(|values| values.len() == 1)
            .map(|values| values[0].as_str())
    };
    let expected = stdlib::websocket::accept_key(key).map_err(error)?;
    Ok(
        single("upgrade").is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
            && single("connection").is_some_and(|value| {
                value
                    .split(',')
                    .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
            })
            && single("sec-websocket-accept") == Some(expected.as_str()),
    )
}
fn http_response_map(status: i64, content_type: &str, body: Vec<u8>) -> Value {
    Value::Map(BTreeMap::from([
        ("status".into(), Value::Int(status)),
        (
            "headers".into(),
            Value::Map(BTreeMap::from([(
                "Content-Type".into(),
                Value::Str(content_type.into()),
            )])),
        ),
        ("body".into(), Value::Bytes(body)),
        ("keep_alive".into(), Value::Bool(true)),
    ]))
}
static REQUEST_IDS: AtomicU64 = AtomicU64::new(1);
const MAX_RATE_LIMIT_KEYS_PER_RUNTIME: usize = 4_096;
const MAX_RATE_LIMIT_KEY_BYTES: usize = 256;
static RATE_LIMITS: OnceLock<Mutex<HashMap<(u64, String), (Instant, u64)>>> = OnceLock::new();
fn with_response_headers(
    mut response: Value,
    update: impl FnOnce(&mut BTreeMap<String, Value>) -> Result<(), String>,
) -> Result<Value, String> {
    let Value::Map(response_map) = &mut response else {
        return Err("HTTP response must be map".into());
    };
    let headers = response_map
        .entry("headers".into())
        .or_insert_with(|| Value::Map(BTreeMap::new()));
    let Value::Map(headers) = headers else {
        return Err("HTTP response headers must be map".into());
    };
    update(headers)?;
    Ok(response)
}
fn rate_limit(runtime_id: u64, key: &str, maximum: u64, window: Duration) -> Result<bool, String> {
    if maximum == 0 || window.is_zero() {
        return Ok(false);
    }
    if key.is_empty() || key.len() > MAX_RATE_LIMIT_KEY_BYTES {
        return Err("invalid rate limit key".into());
    }
    let now = Instant::now();
    let mut limits = RATE_LIMITS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "rate limit registry poisoned")?;
    let owned_key = (runtime_id, key.to_string());
    if !limits.contains_key(&owned_key)
        && limits
            .keys()
            .filter(|(owner, _)| *owner == runtime_id)
            .count()
            >= MAX_RATE_LIMIT_KEYS_PER_RUNTIME
    {
        return Err("rate limit key quota exceeded".into());
    }
    let entry = limits.entry(owned_key).or_insert((now, 0));
    if now.duration_since(entry.0) >= window {
        *entry = (now, 0);
    }
    if entry.1 >= maximum {
        return Ok(false);
    }
    entry.1 += 1;
    Ok(true)
}

pub(crate) fn cleanup_runtime_resources(runtime_id: u64) -> usize {
    let Some(limits) = RATE_LIMITS.get() else {
        return 0;
    };
    let mut limits = limits
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let before = limits.len();
    limits.retain(|(owner, _), _| *owner != runtime_id);
    before - limits.len()
}

fn require_capability(
    name: &str,
    capability: Capability,
    caps: RuntimeCapabilities,
) -> Result<(), VmError> {
    let allowed = match capability {
        Capability::None => true,
        Capability::Filesystem => caps.filesystem,
        Capability::Process => caps.process,
        Capability::Network => caps.network,
        Capability::Environment => caps.environment,
        Capability::UserInterface => caps.user_interface,
        Capability::FilesystemUserInterface => caps.filesystem && caps.user_interface,
    };
    if allowed {
        Ok(())
    } else {
        Err(VmError::PermissionDenied {
            function: name.into(),
            capability: format!("{capability:?}"),
        })
    }
}
fn failure(function: &str, message: &str) -> VmError {
    VmError::Native {
        function: function.into(),
        message: message.into(),
    }
}
fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn expect_string(value: Value) -> Result<String, String> {
    if let Value::Str(v) = value {
        Ok(v)
    } else {
        Err("expected string".into())
    }
}
fn expect_bytes(value: Value) -> Result<Vec<u8>, String> {
    match value {
        Value::Bytes(v) => Ok(v),
        Value::Str(v) => Ok(v.into_bytes()),
        _ => Err("expected bytes or string".into()),
    }
}
fn expect_int(value: Value) -> Result<i64, String> {
    if let Value::Int(v) = value {
        Ok(v)
    } else {
        Err("expected int".into())
    }
}
fn expect_bool(value: Value) -> Result<bool, String> {
    if let Value::Bool(v) = value {
        Ok(v)
    } else {
        Err("expected bool".into())
    }
}
fn expect_float(value: Value) -> Result<f64, String> {
    match value {
        Value::Float(v) => Ok(v),
        Value::Int(v) => Ok(v as f64),
        _ => Err("expected number".into()),
    }
}
fn expect_array(value: Value) -> Result<Vec<Value>, String> {
    match value {
        Value::Array(v) | Value::Tuple(v) => Ok(v),
        _ => Err("expected array".into()),
    }
}
fn expect_map(value: Value) -> Result<BTreeMap<String, Value>, String> {
    if let Value::Map(values) = value {
        Ok(values)
    } else {
        Err("expected map".into())
    }
}
fn expect_string_array(value: Value) -> Result<Vec<String>, String> {
    expect_array(value)?
        .into_iter()
        .map(expect_string)
        .collect()
}
fn strings(values: Vec<Value>) -> Result<Vec<String>, String> {
    values.into_iter().map(expect_string).collect()
}
fn numbers(values: Vec<Value>) -> Result<Vec<f64>, String> {
    values.into_iter().map(expect_float).collect()
}
fn nonnegative(value: i64) -> Result<usize, String> {
    usize::try_from(value).map_err(|_| "expected nonnegative integer".into())
}
fn to_i64(value: usize) -> Result<i64, String> {
    i64::try_from(value).map_err(error)
}
fn checked_float(value: f64, message: &str) -> Result<Value, String> {
    if value.is_finite() {
        Ok(Value::Float(value))
    } else {
        Err(message.into())
    }
}
fn optional_string(value: Option<String>) -> Value {
    value.map(Value::Str).unwrap_or(Value::Nil)
}
fn optional_path(value: Option<std::path::PathBuf>) -> Value {
    value
        .map(|p| Value::Str(p.to_string_lossy().into()))
        .unwrap_or(Value::Nil)
}
fn value_length(value: &Value) -> Result<usize, String> {
    match value {
        Value::Str(v) => Ok(v.chars().count()),
        Value::Bytes(v) => Ok(v.len()),
        Value::Array(v) | Value::Tuple(v) => Ok(v.len()),
        Value::Map(v) => Ok(v.len()),
        _ => Err("value has no length".into()),
    }
}
fn process_output(output: stdlib::process::ProcessOutput) -> Value {
    let mut map = BTreeMap::new();
    map.insert(
        "status".into(),
        output
            .status
            .map(|v| Value::Int(i64::from(v)))
            .unwrap_or(Value::Nil),
    );
    map.insert("success".into(), Value::Bool(output.success));
    map.insert("stdout".into(), Value::Bytes(output.stdout));
    map.insert("stderr".into(), Value::Bytes(output.stderr));
    map.insert("timed_out".into(), Value::Bool(output.timed_out));
    Value::Map(map)
}

#[cfg(feature = "http_full_mod")]
fn build_http_full_options(
    options: &mut stdlib::http_full_mod::Options,
    mut map: BTreeMap<String, Value>,
) -> Result<(), String> {
    if let Some(user) = map.remove("basic_user").map(expect_string).transpose()? {
        let pass = map
            .remove("basic_pass")
            .map(expect_string)
            .transpose()?
            .unwrap_or_default();
        options.basic_auth = Some((user, pass));
    }
    if let Some(token) = map.remove("bearer").map(expect_string).transpose()? {
        options.bearer = Some(token);
    }
    if let Some(agent_name) = map.remove("user_agent").map(expect_string).transpose()? {
        options.user_agent = Some(agent_name);
    }
    if let Some(Value::Int(millis)) = map.remove("timeout_ms") {
        if millis >= 0 {
            options.timeout_ms = Some(millis as u64);
        }
    }
    if let Some(Value::Int(redirects)) = map.remove("max_redirects") {
        if let Ok(n) = u32::try_from(redirects) {
            options.max_redirects = Some(n);
        }
    }
    Ok(())
}

#[cfg(feature = "http_full_mod")]
fn call_http_full(
    method: &str,
    url: &str,
    body: &[u8],
    options: &stdlib::http_full_mod::Options,
) -> Result<stdlib::http_full_mod::Response, String> {
    let method_upper = method.to_ascii_uppercase();
    match method_upper.as_str() {
        "GET" => stdlib::http_full_mod::get(url, options),
        "HEAD" => stdlib::http_full_mod::head(url, options),
        "DELETE" => stdlib::http_full_mod::delete(url, options),
        "POST" => stdlib::http_full_mod::post(url, body, options),
        "PUT" => stdlib::http_full_mod::put(url, body, options),
        "PATCH" => stdlib::http_full_mod::patch(url, body, options),
        other => {
            return Err(format!(
                "std::http_full::request unsupported method '{other}'"
            ))
        }
    }
    .map_err(|e| e.to_string())
}

#[cfg(feature = "http_full_mod")]
fn http_full_response_to_value(response: stdlib::http_full_mod::Response) -> Value {
    let mut map = BTreeMap::new();
    map.insert("status".into(), Value::Int(response.status as i64));
    map.insert(
        "headers".into(),
        Value::Map(
            response
                .headers
                .into_iter()
                .map(|(k, v)| (k, Value::Str(v)))
                .collect(),
        ),
    );
    map.insert("body".into(), Value::Bytes(response.body));
    map.insert("final_url".into(), Value::Str(response.final_url));
    Value::Map(map)
}

#[cfg(feature = "archive_mod")]
fn value_to_archive_entries(
    value: Value,
) -> Result<Vec<stdlib::archive_mod::ArchiveEntry>, String> {
    let items = expect_array(value)?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let mut map = expect_map(item)?;
        let name = map
            .remove("name")
            .ok_or_else(|| "archive entry map requires 'name'".to_string())
            .and_then(expect_string)?;
        let bytes = map
            .remove("bytes")
            .ok_or_else(|| "archive entry map requires 'bytes'".to_string())
            .and_then(expect_bytes)?;
        out.push(stdlib::archive_mod::ArchiveEntry { name, bytes });
    }
    Ok(out)
}

#[cfg(feature = "archive_mod")]
fn archive_entries_to_value(entries: Vec<stdlib::archive_mod::ArchiveEntry>) -> Value {
    Value::Array(
        entries
            .into_iter()
            .map(|entry| {
                let mut map = BTreeMap::new();
                map.insert("name".into(), Value::Str(entry.name));
                map.insert("bytes".into(), Value::Bytes(entry.bytes));
                Value::Map(map)
            })
            .collect(),
    )
}

#[cfg(feature = "audio_mod")]
fn audio_read_result(samples: Vec<f32>, sample_rate: u32, channels: u16, bits: u16) -> Value {
    let mut map = BTreeMap::new();
    map.insert(
        "samples".into(),
        Value::Array(
            samples
                .into_iter()
                .map(|value| Value::Float(value as f64))
                .collect(),
        ),
    );
    map.insert("sample_rate".into(), Value::Int(sample_rate as i64));
    map.insert("channels".into(), Value::Int(channels as i64));
    map.insert("bits_per_sample".into(), Value::Int(bits as i64));
    Value::Map(map)
}

#[cfg(feature = "audio_mod")]
fn audio_samples_from_array(items: Vec<Value>) -> Result<Vec<f32>, String> {
    items
        .into_iter()
        .map(|value| match value {
            Value::Float(f) => Ok(f as f32),
            Value::Int(i) => Ok(i as f32),
            other => Err(format!(
                "audio sample array must contain numbers, got {other:?}"
            )),
        })
        .collect()
}

#[cfg(feature = "audio_mod")]
fn audio_samples_to_array(samples: Vec<f32>) -> Value {
    Value::Array(
        samples
            .into_iter()
            .map(|value| Value::Float(value as f64))
            .collect(),
    )
}

fn to_json(value: Value) -> Result<serde_json::Value, String> {
    Ok(match value {
        Value::Nil => serde_json::Value::Null,
        Value::Bool(v) => v.into(),
        Value::Int(v) => v.into(),
        Value::Float(v) => serde_json::Number::from_f64(v)
            .map(serde_json::Value::Number)
            .ok_or("non-finite JSON number")?,
        Value::Char(v) => v.to_string().into(),
        Value::Str(v) => v.into(),
        Value::Bytes(v) => serde_json::Value::Array(
            v.into_iter()
                .map(|x| serde_json::Value::from(u64::from(x)))
                .collect(),
        ),
        Value::Array(v) | Value::Tuple(v) => {
            serde_json::Value::Array(v.into_iter().map(to_json).collect::<Result<_, _>>()?)
        }
        Value::Map(v) | Value::Struct { fields: v, .. } => serde_json::Value::Object(
            v.into_iter()
                .map(|(k, v)| Ok((k, to_json(v)?)))
                .collect::<Result<_, String>>()?,
        ),
        Value::Enum {
            name,
            variant,
            payload,
        } => {
            let mut map = serde_json::Map::new();
            map.insert("type".into(), name.into());
            map.insert("variant".into(), variant.into());
            if let Some(value) = payload {
                map.insert("payload".into(), to_json(*value)?);
            }
            serde_json::Value::Object(map)
        }
        Value::Closure { .. }
        | Value::Task(_)
        | Value::ChannelSender(_)
        | Value::ChannelReceiver(_)
        | Value::TcpListener(_)
        | Value::TcpStream(_)
        | Value::HttpRouter(_)
        | Value::TlsStream(_)
        | Value::TlsServerConfig(_)
        | Value::WebSocketDecoder(_)
        | Value::WebSocket(_)
        | Value::ServerControl(_)
        | Value::Sqlite(_)
        | Value::SqlitePool(_)
        | Value::Postgres(_)
        | Value::PostgresPool(_)
        | Value::Mysql(_)
        | Value::MysqlPool(_) => return Err("runtime handles cannot be encoded as JSON".into()),
    })
}
fn from_json(value: serde_json::Value) -> Result<Value, String> {
    Ok(match value {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(v) => Value::Bool(v),
        serde_json::Value::Number(v) => {
            if let Some(i) = v.as_i64() {
                Value::Int(i)
            } else {
                Value::Float(v.as_f64().ok_or("invalid JSON number")?)
            }
        }
        serde_json::Value::String(v) => Value::Str(v),
        serde_json::Value::Array(v) => {
            Value::Array(v.into_iter().map(from_json).collect::<Result<_, _>>()?)
        }
        serde_json::Value::Object(v) => Value::Map(
            v.into_iter()
                .map(|(k, v)| Ok((k, from_json(v)?)))
                .collect::<Result<_, String>>()?,
        ),
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
pub fn titan_game_check_collision(
    pos1: (f64, f64),
    size1: (f64, f64),
    pos2: (f64, f64),
    size2: (f64, f64),
) -> bool {
    titan_stdlib::game::check_collision(pos1, size1, pos2, size2)
}
pub fn titan_game_shutdown() -> bool {
    titan_stdlib::game::shutdown()
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
pub fn titan_gui_add_button(
    parent_id: i64,
    label: &str,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
) -> i64 {
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

// --- Phase 9: Freestanding & Bare-Metal Bindings ---
pub fn titan_freestanding_init(target_arch: &str) -> bool {
    titan_stdlib::freestanding::init(target_arch)
}
pub fn titan_freestanding_validate_target_spec(target: &str) -> bool {
    titan_stdlib::freestanding::validate_target_spec(target)
}
pub fn titan_freestanding_generate_linker_script(
    target_arch: &str,
    base_addr: u64,
    stack_size: u64,
) -> String {
    titan_stdlib::freestanding::generate_linker_script(target_arch, base_addr, stack_size)
}
pub fn titan_freestanding_generate_startup_asm(target_arch: &str, entry_fn: &str) -> String {
    titan_stdlib::freestanding::generate_startup_asm(target_arch, entry_fn)
}
pub fn titan_freestanding_get_active_target() -> String {
    titan_stdlib::freestanding::get_active_target()
}
pub fn titan_freestanding_shutdown() -> bool {
    titan_stdlib::freestanding::shutdown()
}

// --- Phase 9: Freestanding Memory & Paging Bindings ---
pub fn titan_freestanding_memory_init_frame_allocator(
    base_paddr: u64,
    total_size_bytes: u64,
) -> bool {
    titan_stdlib::freestanding_memory::init_frame_allocator(base_paddr, total_size_bytes)
}
pub fn titan_freestanding_memory_allocate_frame() -> u64 {
    titan_stdlib::freestanding_memory::allocate_frame()
}
pub fn titan_freestanding_memory_deallocate_frame(paddr: u64) -> bool {
    titan_stdlib::freestanding_memory::deallocate_frame(paddr)
}
pub fn titan_freestanding_memory_map_page(vaddr: u64, paddr: u64, flags: u32) -> bool {
    titan_stdlib::freestanding_memory::map_page(vaddr, paddr, flags)
}
pub fn titan_freestanding_memory_translate_page(vaddr: u64) -> u64 {
    titan_stdlib::freestanding_memory::translate_page(vaddr)
}
pub fn titan_freestanding_memory_free_frames_count() -> u64 {
    titan_stdlib::freestanding_memory::free_frames_count()
}
pub fn titan_freestanding_memory_shutdown() -> bool {
    titan_stdlib::freestanding_memory::shutdown()
}

// --- Phase 9: Freestanding CPU & Exception Traps Bindings ---
pub fn titan_freestanding_cpu_init_exception_table(base_vbar: u64) -> bool {
    titan_stdlib::freestanding_cpu::init_exception_table(base_vbar)
}
pub fn titan_freestanding_cpu_register_exception_handler(
    vector_id: u32,
    handler_vaddr: u64,
) -> bool {
    titan_stdlib::freestanding_cpu::register_exception_handler(vector_id, handler_vaddr)
}
pub fn titan_freestanding_cpu_dispatch_exception(
    vector_id: u32,
    fault_addr: u64,
    error_code: u64,
) -> u64 {
    titan_stdlib::freestanding_cpu::dispatch_exception(vector_id, fault_addr, error_code)
}
pub fn titan_freestanding_cpu_register_syscall_handler(
    syscall_num: u32,
    handler_vaddr: u64,
) -> bool {
    titan_stdlib::freestanding_cpu::register_syscall_handler(syscall_num, handler_vaddr)
}
pub fn titan_freestanding_cpu_invoke_syscall(
    syscall_num: u32,
    arg0: u64,
    arg1: u64,
    arg2: u64,
) -> u64 {
    titan_stdlib::freestanding_cpu::invoke_syscall(syscall_num, arg0, arg1, arg2)
}
pub fn titan_freestanding_cpu_get_last_fault_addr() -> u64 {
    titan_stdlib::freestanding_cpu::get_last_fault_addr()
}
pub fn titan_freestanding_cpu_shutdown() -> bool {
    titan_stdlib::freestanding_cpu::shutdown()
}

// --- Phase 9: Freestanding MMIO & UART Serial Bindings ---
pub fn titan_freestanding_mmio_init_mmio_region(base_paddr: u64, size_bytes: u64) -> bool {
    titan_stdlib::freestanding_mmio::init_mmio_region(base_paddr, size_bytes)
}
pub fn titan_freestanding_mmio_read_mmio_u32(paddr: u64) -> u32 {
    titan_stdlib::freestanding_mmio::read_mmio_u32(paddr)
}
pub fn titan_freestanding_mmio_write_mmio_u32(paddr: u64, value: u32) -> bool {
    titan_stdlib::freestanding_mmio::write_mmio_u32(paddr, value)
}
pub fn titan_freestanding_mmio_serial_init(uart_base_paddr: u64, baudrate: u32) -> bool {
    titan_stdlib::freestanding_mmio::serial_init(uart_base_paddr, baudrate)
}
pub fn titan_freestanding_mmio_serial_write_str(text: &str) -> usize {
    titan_stdlib::freestanding_mmio::serial_write_str(text)
}
pub fn titan_freestanding_mmio_serial_get_buffer() -> String {
    titan_stdlib::freestanding_mmio::serial_get_buffer()
}
pub fn titan_freestanding_mmio_shutdown() -> bool {
    titan_stdlib::freestanding_mmio::shutdown()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_native_bindings() {
        let win_id = invoke(
            "std::window::create",
            vec![
                Value::Str("VM Window".into()),
                Value::Int(800),
                Value::Int(600),
            ],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert!(matches!(win_id, Value::Int(i) if i > 0));
        if let Value::Int(id) = win_id {
            let is_open = invoke(
                "std::window::is_open",
                vec![Value::Int(id)],
                RuntimeCapabilities::all(),
            )
            .unwrap();
            assert_eq!(is_open, Value::Bool(true));
            let closed = invoke(
                "std::window::close",
                vec![Value::Int(id)],
                RuntimeCapabilities::all(),
            )
            .unwrap();
            assert_eq!(closed, Value::Bool(true));
        }
    }

    #[test]
    fn window_bindings_reject_wrapping_numeric_inputs() {
        let capabilities = RuntimeCapabilities::all();
        assert!(invoke(
            "std::window::create",
            vec![
                Value::Str("invalid".into()),
                Value::Int(-1),
                Value::Int(600),
            ],
            capabilities,
        )
        .is_err());
        assert!(invoke(
            "std::window::resize",
            vec![Value::Int(1), Value::Int(i64::MAX), Value::Int(600)],
            capabilities,
        )
        .is_err());
        assert!(invoke("std::window::close", vec![Value::Int(-1)], capabilities,).is_err());
    }

    #[test]
    fn test_input_clipboard_native_bindings() {
        stdlib::input::set_key_state("Enter", true);
        let pressed = invoke(
            "std::input::is_key_pressed",
            vec![Value::Str("Enter".into())],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(pressed, Value::Bool(true));

        invoke(
            "std::clipboard::set_text",
            vec![Value::Str("Copied Data".into())],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        let clip = invoke(
            "std::clipboard::get_text",
            vec![],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(clip, Value::Str("Copied Data".into()));

        let notified = invoke(
            "std::notify::send",
            vec![Value::Str("Alert".into()), Value::Str("Done".into())],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(notified, Value::Bool(true));
    }

    #[test]
    fn test_mobile_native_bindings() {
        invoke(
            "std::mobile::trigger",
            vec![Value::Str("onPause".into())],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        let state = invoke("std::mobile::state", vec![], RuntimeCapabilities::all()).unwrap();
        assert_eq!(state, Value::Str("Paused".into()));

        let events = invoke(
            "std::mobile::poll_events",
            vec![],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert!(matches!(events, Value::Array(v) if !v.is_empty()));
    }

    #[test]
    fn test_game_audio_native_bindings() {
        let init_game = invoke(
            "std::game::init",
            vec![
                Value::Str("VM Game".into()),
                Value::Int(800),
                Value::Int(600),
            ],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(init_game, Value::Bool(true));

        let _ = invoke("std::game::step", vec![], RuntimeCapabilities::all()).unwrap();
        let _ = invoke("std::game::fps", vec![], RuntimeCapabilities::all()).unwrap();

        let coll = invoke(
            "std::game::check_collision",
            vec![
                Value::Float(0.0),
                Value::Float(0.0),
                Value::Float(10.0),
                Value::Float(10.0),
                Value::Float(2.0),
                Value::Float(2.0),
                Value::Float(10.0),
                Value::Float(10.0),
            ],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(coll, Value::Bool(true));

        assert_eq!(
            invoke("std::audio::sim_init", vec![], RuntimeCapabilities::all()).unwrap(),
            Value::Bool(true)
        );
        let handle_val = invoke(
            "std::audio::sim_load_wave",
            vec![Value::Float(220.0), Value::Int(50)],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        if let Value::Int(handle) = handle_val {
            let _ = invoke(
                "std::audio::sim_sample_count",
                vec![Value::Int(handle)],
                RuntimeCapabilities::all(),
            )
            .unwrap();
            let _ = invoke(
                "std::audio::sim_play",
                vec![Value::Int(handle), Value::Bool(true)],
                RuntimeCapabilities::all(),
            )
            .unwrap();
            let _ = invoke(
                "std::audio::sim_set_volume",
                vec![Value::Int(handle), Value::Float(0.8)],
                RuntimeCapabilities::all(),
            )
            .unwrap();
            let _ = invoke(
                "std::audio::sim_stop",
                vec![Value::Int(handle)],
                RuntimeCapabilities::all(),
            )
            .unwrap();
        }
    }
    #[test]
    fn test_gui_native_bindings() {
        assert_eq!(
            invoke("std::gui::init", vec![], RuntimeCapabilities::all()).unwrap(),
            Value::Bool(true)
        );

        let root = invoke(
            "std::gui::create_container",
            vec![
                Value::Str("VM App".into()),
                Value::Int(1024),
                Value::Int(768),
            ],
            RuntimeCapabilities::all(),
        )
        .unwrap();

        if let Value::Int(root_id) = root {
            assert!(root_id > 0);
            let btn = invoke(
                "std::gui::add_button",
                vec![
                    Value::Int(root_id),
                    Value::Str("Submit".into()),
                    Value::Int(20),
                    Value::Int(20),
                    Value::Int(150),
                    Value::Int(45),
                ],
                RuntimeCapabilities::all(),
            )
            .unwrap();

            if let Value::Int(btn_id) = btn {
                assert!(btn_id > 0);
                assert_eq!(
                    invoke(
                        "std::gui::child_count",
                        vec![Value::Int(root_id)],
                        RuntimeCapabilities::all()
                    )
                    .unwrap(),
                    Value::Int(1)
                );

                assert_eq!(
                    invoke(
                        "std::gui::set_text",
                        vec![Value::Int(btn_id), Value::Str("Send".into())],
                        RuntimeCapabilities::all()
                    )
                    .unwrap(),
                    Value::Bool(true)
                );
                assert_eq!(
                    invoke(
                        "std::gui::get_text",
                        vec![Value::Int(btn_id)],
                        RuntimeCapabilities::all()
                    )
                    .unwrap(),
                    Value::Str("Send".into())
                );

                assert_eq!(
                    invoke(
                        "std::gui::is_clicked",
                        vec![Value::Int(btn_id)],
                        RuntimeCapabilities::all()
                    )
                    .unwrap(),
                    Value::Bool(false)
                );
                assert_eq!(
                    invoke(
                        "std::gui::trigger_click",
                        vec![Value::Int(btn_id)],
                        RuntimeCapabilities::all()
                    )
                    .unwrap(),
                    Value::Bool(true)
                );
                assert_eq!(
                    invoke(
                        "std::gui::is_clicked",
                        vec![Value::Int(btn_id)],
                        RuntimeCapabilities::all()
                    )
                    .unwrap(),
                    Value::Bool(true)
                );
            } else {
                panic!("add_button should return Int handle");
            }
            assert_eq!(
                invoke("std::gui::shutdown", vec![], RuntimeCapabilities::all()).unwrap(),
                Value::Bool(true)
            );
        } else {
            panic!("create_container should return Int handle");
        }
    }
    #[test]
    fn test_freestanding_native_bindings() {
        assert_eq!(
            invoke(
                "std::freestanding::validate_target_spec",
                vec![Value::Str("aarch64-unknown-none".into())],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            invoke(
                "std::freestanding::init",
                vec![Value::Str("aarch64-unknown-none".into())],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            invoke(
                "std::freestanding::get_active_target",
                vec![],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Str("aarch64-unknown-none".into())
        );

        let ld = invoke(
            "std::freestanding::generate_linker_script",
            vec![
                Value::Str("aarch64-unknown-none".into()),
                Value::Int(0x80000),
                Value::Int(0x10000),
            ],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        if let Value::Str(ld_content) = ld {
            assert!(ld_content.contains("ENTRY(_start)"));
            assert!(ld_content.contains(". = 0x80000;"));
        } else {
            panic!("generate_linker_script should return String");
        }

        let asm = invoke(
            "std::freestanding::generate_startup_asm",
            vec![
                Value::Str("aarch64-unknown-none".into()),
                Value::Str("kernel_main".into()),
            ],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        if let Value::Str(asm_content) = asm {
            assert!(asm_content.contains("adrp x0, _stack_top"));
            assert!(asm_content.contains("bl kernel_main"));
        } else {
            panic!("generate_startup_asm should return String");
        }

        assert_eq!(
            invoke(
                "std::freestanding::shutdown",
                vec![],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );
    }
    #[test]
    fn test_freestanding_memory_native_bindings() {
        assert_eq!(
            invoke(
                "std::freestanding_memory::init_frame_allocator",
                vec![
                    Value::Int(0x200000),
                    Value::Int(0x8000) // 32KB = 8 frames de 4KB
                ],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );

        assert_eq!(
            invoke(
                "std::freestanding_memory::free_frames_count",
                vec![],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Int(8)
        );

        let frame = invoke(
            "std::freestanding_memory::allocate_frame",
            vec![],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        if let Value::Int(paddr) = frame {
            assert_eq!(paddr, 0x200000);
            assert_eq!(
                invoke(
                    "std::freestanding_memory::free_frames_count",
                    vec![],
                    RuntimeCapabilities::all()
                )
                .unwrap(),
                Value::Int(7)
            );

            assert_eq!(
                invoke(
                    "std::freestanding_memory::map_page",
                    vec![Value::Int(0x80000000), Value::Int(paddr), Value::Int(3)],
                    RuntimeCapabilities::all()
                )
                .unwrap(),
                Value::Bool(true)
            );

            assert_eq!(
                invoke(
                    "std::freestanding_memory::translate_page",
                    vec![Value::Int(0x80000010)],
                    RuntimeCapabilities::all()
                )
                .unwrap(),
                Value::Int(0x200010)
            );

            assert_eq!(
                invoke(
                    "std::freestanding_memory::deallocate_frame",
                    vec![Value::Int(paddr)],
                    RuntimeCapabilities::all()
                )
                .unwrap(),
                Value::Bool(true)
            );
            assert_eq!(
                invoke(
                    "std::freestanding_memory::free_frames_count",
                    vec![],
                    RuntimeCapabilities::all()
                )
                .unwrap(),
                Value::Int(8)
            );
        } else {
            panic!("allocate_frame should return Int");
        }

        assert_eq!(
            invoke(
                "std::freestanding_memory::shutdown",
                vec![],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );
    }
    #[test]
    fn test_freestanding_cpu_native_bindings() {
        assert_eq!(
            invoke(
                "std::freestanding_cpu::init_exception_table",
                vec![Value::Int(0x8000_0000)],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );

        assert_eq!(
            invoke(
                "std::freestanding_cpu::register_exception_handler",
                vec![Value::Int(0), Value::Int(0xFFFF_0000_8000_1000u64 as i64)],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );

        assert_eq!(
            invoke(
                "std::freestanding_cpu::dispatch_exception",
                vec![Value::Int(0), Value::Int(0x4000_1234), Value::Int(0x05)],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Int((0xFFFF_0000_8000_1000u64 ^ 0x4000_1234u64 ^ 0x05u64) as i64)
        );

        assert_eq!(
            invoke(
                "std::freestanding_cpu::get_last_fault_addr",
                vec![],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Int(0x4000_1234)
        );

        assert_eq!(
            invoke(
                "std::freestanding_cpu::register_syscall_handler",
                vec![Value::Int(1), Value::Int(0x9000_0000)],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );

        assert_eq!(
            invoke(
                "std::freestanding_cpu::invoke_syscall",
                vec![
                    Value::Int(1),
                    Value::Int(10),
                    Value::Int(20),
                    Value::Int(30)
                ],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Int(0x9000_0000 + 60)
        );

        assert_eq!(
            invoke(
                "std::freestanding_cpu::shutdown",
                vec![],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );
    }
    #[test]
    fn test_freestanding_mmio_and_kernel_demo() {
        // 1. Inicializar región MMIO genérica y verificar lectura/escritura volátil
        assert_eq!(
            invoke(
                "std::freestanding_mmio::init_mmio_region",
                vec![Value::Int(0x3F00_0000), Value::Int(0x1000)],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );

        assert_eq!(
            invoke(
                "std::freestanding_mmio::write_mmio_u32",
                vec![Value::Int(0x3F00_0004), Value::Int(0x1234_5678)],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            invoke(
                "std::freestanding_mmio::read_mmio_u32",
                vec![Value::Int(0x3F00_0004)],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Int(0x1234_5678)
        );

        // 2. Inicializar puerto serial UART bare-metal (0x1000_0000 en ARM64 PL011) a 115200 baudios
        assert_eq!(
            invoke(
                "std::freestanding_mmio::serial_init",
                vec![Value::Int(0x1000_0000), Value::Int(115200)],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );

        // 3. Simular secuencia real de arranque de un Demo Kernel bare-metal escrito en TITAN
        assert_eq!(
            invoke(
                "std::freestanding_mmio::serial_write_str",
                vec![Value::Str(
                    "[BOOT] TITAN Bare-Metal Kernel Starting...
"
                    .into()
                )],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Int(43)
        );

        assert_eq!(
            invoke(
                "std::freestanding_mmio::serial_write_str",
                vec![Value::Str(
                    "[MMIO] UART PL011 Serial Driver Online.
"
                    .into()
                )],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Int(40)
        );

        let buffer = invoke(
            "std::freestanding_mmio::serial_get_buffer",
            vec![],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(
            buffer,
            Value::Str(
                "[BOOT] TITAN Bare-Metal Kernel Starting...
[MMIO] UART PL011 Serial Driver Online.
"
                .into()
            )
        );

        assert_eq!(
            invoke(
                "std::freestanding_mmio::shutdown",
                vec![],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );
    }

    // ------------------------------------------------------------------
    // Phase 1 extras — end-to-end VM bindings
    // ------------------------------------------------------------------
    #[cfg(feature = "regex_mod")]
    #[test]
    fn regex_native_bindings() {
        let out = invoke(
            "std::regex::find_all",
            vec![Value::Str(r"\d+".into()), Value::Str("a1 b22 c333".into())],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(
            out,
            Value::Array(vec![
                Value::Str("1".into()),
                Value::Str("22".into()),
                Value::Str("333".into()),
            ])
        );

        let replaced = invoke(
            "std::regex::replace_all",
            vec![
                Value::Str(r"\s+".into()),
                Value::Str("hola   mundo".into()),
                Value::Str("_".into()),
            ],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(replaced, Value::Str("hola_mundo".into()));
    }

    #[cfg(feature = "uuid_mod")]
    #[test]
    fn uuid_native_bindings() {
        let Value::Str(a) = invoke("std::uuid::v4", vec![], RuntimeCapabilities::all()).unwrap()
        else {
            panic!()
        };
        assert_eq!(a.len(), 36);
        assert_eq!(
            invoke(
                "std::uuid::is_valid",
                vec![Value::Str(a.clone())],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            invoke("std::uuid::nil", vec![], RuntimeCapabilities::all()).unwrap(),
            Value::Str("00000000-0000-0000-0000-000000000000".into())
        );
    }

    #[cfg(feature = "hash_mod")]
    #[test]
    fn hash_native_bindings() {
        assert_eq!(
            invoke(
                "std::hash::sha256",
                vec![Value::Bytes(b"abc".to_vec())],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Str("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into())
        );
        assert_eq!(
            invoke(
                "std::hash::blake3",
                vec![Value::Bytes(b"abc".to_vec())],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Str("6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85".into())
        );
    }

    #[cfg(feature = "random_mod")]
    #[test]
    fn random_native_bindings() {
        let out = invoke(
            "std::random::range",
            vec![Value::Int(1), Value::Int(10)],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        if let Value::Int(v) = out {
            assert!((1..=10).contains(&v));
        } else {
            panic!("expected Int");
        }

        let a = invoke(
            "std::random::seeded_bytes",
            vec![Value::Int(42), Value::Int(8)],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        let b = invoke(
            "std::random::seeded_bytes",
            vec![Value::Int(42), Value::Int(8)],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(a, b, "seeded_bytes must be deterministic");
    }

    #[cfg(feature = "datetime_mod")]
    #[test]
    fn datetime_native_bindings() {
        let ts = invoke(
            "std::datetime::utc_ymd_hms",
            vec![
                Value::Int(2026),
                Value::Int(7),
                Value::Int(25),
                Value::Int(12),
                Value::Int(0),
                Value::Int(0),
            ],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(ts, Value::Int(1_784_980_800));
        let rfc = invoke(
            "std::datetime::to_rfc3339",
            vec![ts.clone()],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(rfc, Value::Str("2026-07-25T12:00:00+00:00".into()));
        let parsed = invoke(
            "std::datetime::parse_rfc3339",
            vec![rfc],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(parsed, ts);
    }

    #[cfg(feature = "url_mod")]
    #[test]
    fn url_native_bindings() {
        let url = Value::Str("https://user@example.com:8443/api?q=hola+mundo&n=1#frag".into());
        assert_eq!(
            invoke(
                "std::url::scheme",
                vec![url.clone()],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Str("https".into())
        );
        assert_eq!(
            invoke(
                "std::url::host",
                vec![url.clone()],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Str("example.com".into())
        );
        assert_eq!(
            invoke(
                "std::url::port",
                vec![url.clone()],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Int(8443)
        );
        let built = invoke(
            "std::url::build_query",
            vec![Value::Array(vec![
                Value::Array(vec![
                    Value::Str("q".into()),
                    Value::Str("hola mundo".into()),
                ]),
                Value::Array(vec![Value::Str("n".into()), Value::Str("1".into())]),
            ])],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(built, Value::Str("q=hola+mundo&n=1".into()));
    }

    #[cfg(feature = "dirs_mod")]
    #[test]
    fn dirs_native_bindings() {
        let Value::Str(temp) =
            invoke("std::dirs::temp", vec![], RuntimeCapabilities::all()).unwrap()
        else {
            panic!()
        };
        assert!(!temp.is_empty());
        let Value::Str(cwd) =
            invoke("std::dirs::current", vec![], RuntimeCapabilities::all()).unwrap()
        else {
            panic!()
        };
        assert!(!cwd.is_empty());
    }

    // -----------------------------------------------------------------
    // Phase 2 extras — end-to-end VM bindings
    // -----------------------------------------------------------------
    #[cfg(feature = "compress_mod")]
    #[test]
    fn compress_native_bindings_round_trip() {
        let data = Value::Bytes(b"hola mundo hola mundo hola mundo".to_vec());
        for (encoder, decoder, level) in [
            (
                "std::compress::gzip_encode",
                "std::compress::gzip_decode",
                6,
            ),
            (
                "std::compress::zlib_encode",
                "std::compress::zlib_decode",
                6,
            ),
            (
                "std::compress::deflate_encode",
                "std::compress::deflate_decode",
                6,
            ),
            (
                "std::compress::zstd_encode",
                "std::compress::zstd_decode",
                3,
            ),
        ] {
            let encoded = invoke(
                encoder,
                vec![data.clone(), Value::Int(level)],
                RuntimeCapabilities::all(),
            )
            .unwrap();
            let decoded =
                invoke(decoder, vec![encoded.clone()], RuntimeCapabilities::all()).unwrap();
            assert_eq!(decoded, data, "{encoder}/{decoder} round-trip failed");
        }
    }

    #[cfg(feature = "archive_mod")]
    #[test]
    fn archive_native_bindings_round_trip() {
        let entry_map = |name: &str, bytes: Vec<u8>| {
            let mut map = BTreeMap::new();
            map.insert("name".to_string(), Value::Str(name.into()));
            map.insert("bytes".to_string(), Value::Bytes(bytes));
            Value::Map(map)
        };
        let entries = Value::Array(vec![
            entry_map("hola.txt", b"hola".to_vec()),
            entry_map("mundo.txt", b"mundo".to_vec()),
        ]);

        // tar
        let packed = invoke(
            "std::archive::tar_pack",
            vec![entries.clone()],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        let unpacked = invoke(
            "std::archive::tar_unpack",
            vec![packed],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        let Value::Array(items) = unpacked else {
            panic!("tar unpack should return array");
        };
        assert_eq!(items.len(), 2);

        // zip
        let packed = invoke(
            "std::archive::zip_pack",
            vec![entries.clone()],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        let names = invoke(
            "std::archive::zip_list",
            vec![packed.clone()],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(
            names,
            Value::Array(vec![
                Value::Str("hola.txt".into()),
                Value::Str("mundo.txt".into())
            ])
        );
        let unpacked = invoke(
            "std::archive::zip_unpack",
            vec![packed],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        let Value::Array(items) = unpacked else {
            panic!("zip unpack should return array");
        };
        assert_eq!(items.len(), 2);
    }

    #[cfg(feature = "yaml_mod")]
    #[test]
    fn yaml_native_bindings_round_trip() {
        let doc = invoke(
            "std::yaml::parse",
            vec![Value::Str("name: TITAN\nversion: 2\n".into())],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        let Value::Map(map) = doc.clone() else {
            panic!("expected map");
        };
        assert_eq!(map.get("name"), Some(&Value::Str("TITAN".into())));
        assert_eq!(map.get("version"), Some(&Value::Int(2)));

        let text = invoke(
            "std::yaml::stringify",
            vec![doc.clone()],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        let Value::Str(text) = text else {
            panic!("expected string");
        };
        let back = invoke(
            "std::yaml::parse",
            vec![Value::Str(text)],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        assert_eq!(back, doc);
    }

    #[cfg(feature = "xml_mod")]
    #[test]
    fn xml_native_bindings_parse_and_escape() {
        let tree = invoke(
            "std::xml::parse",
            vec![Value::Str("<a x=\"1\"><b>hola</b></a>".into())],
            RuntimeCapabilities::all(),
        )
        .unwrap();
        let Value::Map(map) = tree else {
            panic!("expected map");
        };
        assert_eq!(map.get("tag"), Some(&Value::Str("a".into())));

        assert_eq!(
            invoke(
                "std::xml::escape_text",
                vec![Value::Str("<b>&".into())],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Str("&lt;b&gt;&amp;".into())
        );
        assert_eq!(
            invoke(
                "std::xml::escape_attr",
                vec![Value::Str("a\"b".into())],
                RuntimeCapabilities::all()
            )
            .unwrap(),
            Value::Str("a&quot;b".into())
        );
    }
    #[cfg(feature = "pdf_mod")]
    #[test]
    fn pdf_native_bindings_generate_a_real_file_and_enforce_capability() {
        let capabilities = RuntimeCapabilities::all();
        let Value::Int(handle) = invoke(
            "std::pdf::new",
            vec![
                Value::Str("PDF desde la VM".into()),
                Value::Float(210.0),
                Value::Float(297.0),
            ],
            capabilities,
        )
        .unwrap()
        else {
            panic!("PDF new should return a handle");
        };
        assert_eq!(
            invoke(
                "std::pdf::page_count",
                vec![Value::Int(handle)],
                capabilities
            )
            .unwrap(),
            Value::Int(1)
        );
        invoke(
            "std::pdf::add_text",
            vec![
                Value::Int(handle),
                Value::Int(0),
                Value::Int(0),
                Value::Str("Hola desde TITAN".into()),
                Value::Float(18.0),
                Value::Float(20.0),
                Value::Float(270.0),
            ],
            capabilities,
        )
        .unwrap();
        invoke(
            "std::pdf::set_color",
            vec![
                Value::Int(handle),
                Value::Int(0),
                Value::Int(0),
                Value::Float(0.2),
                Value::Float(0.4),
                Value::Float(0.8),
            ],
            capabilities,
        )
        .unwrap();
        invoke(
            "std::pdf::add_line",
            vec![
                Value::Int(handle),
                Value::Int(0),
                Value::Int(0),
                Value::Float(20.0),
                Value::Float(260.0),
                Value::Float(190.0),
                Value::Float(260.0),
                Value::Float(0.5),
            ],
            capabilities,
        )
        .unwrap();
        invoke(
            "std::pdf::add_rect",
            vec![
                Value::Int(handle),
                Value::Int(0),
                Value::Int(0),
                Value::Float(20.0),
                Value::Float(200.0),
                Value::Float(50.0),
                Value::Float(30.0),
            ],
            capabilities,
        )
        .unwrap();
        assert_eq!(
            invoke(
                "std::pdf::add_page",
                vec![
                    Value::Int(handle),
                    Value::Float(210.0),
                    Value::Float(297.0),
                    Value::Str("Página 2".into()),
                ],
                capabilities,
            )
            .unwrap(),
            Value::Int(1)
        );

        let path = std::env::temp_dir().join(format!(
            "titan-vm-pdf-bindings-{}.pdf",
            std::process::id()
        ));
        let path_string = path.to_string_lossy().into_owned();
        assert!(invoke(
            "std::pdf::save",
            vec![Value::Int(handle), Value::Str(path_string.clone())],
            RuntimeCapabilities::sandboxed(),
        )
        .is_err());
        invoke(
            "std::pdf::save",
            vec![Value::Int(handle), Value::Str(path_string)],
            capabilities,
        )
        .unwrap();
        assert!(std::fs::read(&path).unwrap().starts_with(b"%PDF-"));
        assert_eq!(
            invoke(
                "std::pdf::close",
                vec![Value::Int(handle)],
                capabilities
            )
            .unwrap(),
            Value::Nil
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn rate_limits_are_owned_and_cleaned_by_runtime() {
        let call = |runtime_id| {
            invoke_for_runtime(
                "std::http::rate_limit",
                vec![
                    Value::Str("shared-key".into()),
                    Value::Int(1),
                    Value::Int(60_000),
                ],
                RuntimeCapabilities::all(),
                runtime_id,
            )
            .unwrap()
        };
        assert_eq!(call(82_001), Value::Bool(true));
        assert_eq!(call(82_001), Value::Bool(false));
        assert_eq!(call(82_002), Value::Bool(true));
        assert_eq!(cleanup_runtime_resources(82_001), 1);
        assert_eq!(call(82_001), Value::Bool(true));
        assert_eq!(cleanup_runtime_resources(82_001), 1);
        assert_eq!(cleanup_runtime_resources(82_002), 1);
    }
    #[test]
    fn rate_limit_key_quota_is_finite_and_cleanup_recovers_it() {
        let runtime_id = 85_010;
        for index in 0..MAX_RATE_LIMIT_KEYS_PER_RUNTIME {
            assert!(rate_limit(
                runtime_id,
                &format!("key-{index}"),
                1,
                Duration::from_secs(60)
            )
            .unwrap());
        }
        assert!(rate_limit(runtime_id, "overflow", 1, Duration::from_secs(60)).is_err());
        assert!(rate_limit(
            runtime_id,
            &"x".repeat(MAX_RATE_LIMIT_KEY_BYTES + 1),
            1,
            Duration::from_secs(60)
        )
        .is_err());
        assert_eq!(
            cleanup_runtime_resources(runtime_id),
            MAX_RATE_LIMIT_KEYS_PER_RUNTIME
        );
        assert!(rate_limit(runtime_id, "recovered", 1, Duration::from_secs(60)).unwrap());
        assert_eq!(cleanup_runtime_resources(runtime_id), 1);
    }
}
