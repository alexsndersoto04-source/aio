use crate::{Position, Range, TitanLsp};
use serde_json::{json, Value};
use std::io::{self, BufRead, Read, Write};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("LSP I/O error: {0}")] Io(#[from] io::Error),
    #[error("invalid JSON-RPC message: {0}")] Json(#[from] serde_json::Error),
    #[error("invalid LSP frame: {0}")] Frame(String),
}

pub fn run_stdio() -> Result<(), ServerError> {
    let stdin = io::stdin(); let stdout = io::stdout();
    run(BufReadAdapter(stdin.lock()), stdout.lock())
}

struct BufReadAdapter<R>(R);
impl<R: Read> Read for BufReadAdapter<R> { fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> { self.0.read(buffer) } }
impl<R: BufRead> BufRead for BufReadAdapter<R> { fn fill_buf(&mut self) -> io::Result<&[u8]> { self.0.fill_buf() } fn consume(&mut self, amount: usize) { self.0.consume(amount); } }

pub fn run<R: BufRead, W: Write>(mut input: R, mut output: W) -> Result<(), ServerError> {
    let mut service = TitanLsp::new(); let mut shutdown = false;
    while let Some(message) = read_message(&mut input)? {
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let id = message.get("id").cloned(); let params = message.get("params").cloned().unwrap_or(Value::Null);
        if let Some(id) = id {
            let result = match method {
                "initialize" => Ok(json!({ "capabilities": { "textDocumentSync": { "openClose": true, "change": 2 }, "completionProvider": { "triggerCharacters": [".", ":"] }, "signatureHelpProvider": { "triggerCharacters": ["(", ","] }, "semanticTokensProvider": { "legend": { "tokenTypes": ["keyword", "function", "type", "variable", "string", "number", "operator"], "tokenModifiers": [] }, "full": true }, "hoverProvider": true, "definitionProvider": true, "referencesProvider": true, "renameProvider": { "prepareProvider": false }, "documentSymbolProvider": true, "workspaceSymbolProvider": true }, "serverInfo": { "name": "titan-lsp", "version": env!("CARGO_PKG_VERSION") } })),
                "shutdown" => { shutdown = true; Ok(Value::Null) }
                "textDocument/completion" => document_position(&params).map(|(uri, _)| Value::Array(service.completions(uri).into_iter().map(|(label, kind, detail)| json!({"label":label,"kind":kind,"detail":detail})).collect())),
                "textDocument/signatureHelp" => document_position(&params).map(|(uri, position)| service.signature_help(uri, position).map(|(label, parameters, active)| json!({"signatures":[{"label":label,"parameters":parameters.into_iter().map(|label| json!({"label":label})).collect::<Vec<_>>() }],"activeSignature":0,"activeParameter":active})).unwrap_or(Value::Null)),
                "textDocument/semanticTokens/full" => document_uri(&params).map(|uri| json!({"data":service.semantic_tokens(uri)})),
                "textDocument/hover" => document_position(&params).map(|(uri, position)| service.hover(uri, position).map(|text| json!({"contents":{"kind":"markdown","value":text}})).unwrap_or(Value::Null)),
                "textDocument/definition" => document_position(&params).map(|(uri, position)| service.definition(uri, position).map(|symbol| json!({"uri":symbol.uri,"range":symbol.selection_range})).unwrap_or(Value::Null)),
                "textDocument/references" => document_position(&params).map(|(uri, position)| Value::Array(service.references(uri, position).into_iter().map(|(uri, range)| json!({"uri":uri,"range":range})).collect())),
                "textDocument/rename" => document_position(&params).and_then(|(uri, position)| { let name = params.get("newName").and_then(Value::as_str).ok_or("missing newName")?; service.rename(uri, position, name).map(|changes| json!({"changes":changes})) }),
                "textDocument/documentSymbol" => document_uri(&params).map(|uri| Value::Array(service.symbols(uri).into_iter().map(|symbol| json!({"name":symbol.name,"detail":symbol.detail,"kind":symbol.kind,"range":symbol.range,"selectionRange":symbol.selection_range})).collect())),
                "workspace/symbol" => Ok(Value::Array(service.workspace_symbols(params.get("query").and_then(Value::as_str).unwrap_or("")).into_iter().map(|symbol| json!({"name":symbol.name,"kind":symbol.kind,"location":{"uri":symbol.uri,"range":symbol.selection_range}})).collect())),
                _ => Err("method not found".into()),
            };
            match result {
                Ok(value) => write_message(&mut output, &json!({"jsonrpc":"2.0","id":id,"result":value}))?,
                Err(message) => write_message(&mut output, &json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":message}}))?,
            }
        } else {
            match method {
                "exit" => break,
                "textDocument/didOpen" => if let Some(document) = params.get("textDocument") { let uri = string(document, "uri")?; service.open_document(uri, string(document, "text")?, document.get("version").and_then(Value::as_i64).unwrap_or(0)); publish(&mut output, &service, uri)?; },
                "textDocument/didChange" => { let document = params.get("textDocument").ok_or_else(|| ServerError::Frame("missing textDocument".into()))?; let uri = string(document, "uri")?; let version = document.get("version").and_then(Value::as_i64).unwrap_or(0); for change in params.get("contentChanges").and_then(Value::as_array).into_iter().flatten() { let range = change.get("range").map(parse_range).transpose().map_err(ServerError::Frame)?; service.apply_change(uri, range, string(change, "text")?, version).map_err(ServerError::Frame)?; } publish(&mut output, &service, uri)?; }
                "textDocument/didClose" => { let uri = document_uri(&params).map_err(ServerError::Frame)?; service.close_document(uri); write_message(&mut output, &json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":[]}}))?; }
                _ => {}
            }
        }
        if shutdown && method == "exit" { break; }
    }
    Ok(())
}

fn publish(output: &mut impl Write, service: &TitanLsp, uri: &str) -> Result<(), ServerError> { write_message(output, &json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":service.diagnostics(uri)}})) }
fn document_uri(params: &Value) -> Result<&str, String> { params.get("textDocument").and_then(|value| value.get("uri")).and_then(Value::as_str).ok_or("missing textDocument.uri".into()) }
fn document_position(params: &Value) -> Result<(&str, Position), String> { let uri = document_uri(params)?; let position = params.get("position").ok_or("missing position")?; Ok((uri, parse_position(position)?)) }
fn parse_position(value: &Value) -> Result<Position, String> { Ok(Position { line: value.get("line").and_then(Value::as_u64).ok_or("missing line")? as u32, character: value.get("character").and_then(Value::as_u64).ok_or("missing character")? as u32 }) }
fn parse_range(value: &Value) -> Result<Range, String> { Ok(Range { start: parse_position(value.get("start").ok_or("missing range.start")?)?, end: parse_position(value.get("end").ok_or("missing range.end")?)? }) }
fn string<'a>(value: &'a Value, name: &str) -> Result<&'a str, ServerError> { value.get(name).and_then(Value::as_str).ok_or_else(|| ServerError::Frame(format!("missing {name}"))) }

fn read_message(input: &mut impl BufRead) -> Result<Option<Value>, ServerError> {
    let mut content_length = None;
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line)? == 0 {
            return if content_length.is_none() {
                Ok(None)
            } else {
                Err(ServerError::Frame("unexpected EOF in headers".into()))
            };
        }

        if line == "\r\n" || line == "\n" {
            break;
        }

        if let Some(value) = line
            .strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("content-length:"))
        {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| ServerError::Frame("invalid Content-Length".into()))?,
            );
        }
    }

    let length = content_length
        .ok_or_else(|| ServerError::Frame("missing Content-Length".into()))?;
    if length > 16 * 1024 * 1024 {
        return Err(ServerError::Frame("message exceeds 16 MiB".into()));
    }

    let mut body = vec![0; length];
    input.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}

fn write_message(output: &mut impl Write, value: &Value) -> Result<(), ServerError> {
    let body = serde_json::to_vec(value)?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()?;
    Ok(())
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn serves_initialize_open_and_symbols_over_framed_json_rpc() { let messages = [json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}), json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///x.titan","version":1,"text":"fn main() { 1 }"}}}), json!({"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":"file:///x.titan"}}}), json!({"jsonrpc":"2.0","method":"exit"})]; let mut input=Vec::new(); for message in messages { let body=serde_json::to_vec(&message).unwrap(); write!(input,"Content-Length: {}\r\n\r\n",body.len()).unwrap(); input.extend(body); } let mut output=Vec::new(); run(io::Cursor::new(input),&mut output).unwrap(); let text=String::from_utf8(output).unwrap(); assert!(text.contains("titan-lsp")); assert!(text.contains("publishDiagnostics")); assert!(text.contains("main")); }
}
