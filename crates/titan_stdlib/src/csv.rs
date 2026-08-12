//! RFC 4180-style CSV parsing and serialization.

use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum CsvError {
    #[error("unterminated quoted field at record {record}")]
    UnterminatedQuote { record: usize },
    #[error("unexpected character after closing quote at byte {byte}")]
    AfterQuote { byte: usize },
    #[error("unexpected quote in unquoted field at byte {byte}")]
    UnexpectedQuote { byte: usize },
}

pub fn parse(input: &str) -> Result<Vec<Vec<String>>, CsvError> {
    let bytes = input.as_bytes();
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut i = 0;
    let mut quoted = false;
    let mut closed_quote = false;
    while i < bytes.len() {
        let c = bytes[i];
        if quoted {
            if c == b'"' {
                if bytes.get(i + 1) == Some(&b'"') {
                    field.push('"');
                    i += 2;
                    continue;
                }
                quoted = false;
                closed_quote = true;
                i += 1;
                continue;
            }
            let ch = input[i..].chars().next().unwrap();
            field.push(ch);
            i += ch.len_utf8();
            continue;
        }
        if closed_quote && !matches!(c, b',' | b'\r' | b'\n') {
            return Err(CsvError::AfterQuote { byte: i });
        }
        match c {
            b'"' if field.is_empty() && !closed_quote => {
                quoted = true;
                i += 1;
            }
            b'"' => return Err(CsvError::UnexpectedQuote { byte: i }),
            b',' => {
                row.push(std::mem::take(&mut field));
                closed_quote = false;
                i += 1;
            }
            b'\r' | b'\n' => {
                if c == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                closed_quote = false;
                i += 1;
            }
            _ => {
                let ch = input[i..].chars().next().unwrap();
                field.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    if quoted {
        return Err(CsvError::UnterminatedQuote {
            record: rows.len() + 1,
        });
    }
    if !field.is_empty() || !row.is_empty() || closed_quote {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

pub fn serialize(rows: &[Vec<String>]) -> String {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|field| encode_field(field))
                .collect::<Vec<_>>()
                .join(",")
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}
fn encode_field(field: &str) -> String {
    if field.chars().any(|c| matches!(c, ',' | '"' | '\r' | '\n')) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.into()
    }
}

#[derive(Debug, Clone)]
pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}
impl Table {
    pub fn from_rows(mut rows: Vec<Vec<String>>) -> Option<Self> {
        if rows.is_empty() {
            None
        } else {
            let headers = rows.remove(0);
            Some(Self { headers, rows })
        }
    }
    pub fn headers(&self) -> &[String] {
        &self.headers
    }
    pub fn rows(&self) -> &[Vec<String>] {
        &self.rows
    }
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.headers.iter().position(|h| h == name)
    }
    pub fn get(&self, row: usize, column: &str) -> Option<&str> {
        self.rows
            .get(row)?
            .get(self.column_index(column)?)
            .map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quoted_round_trip() {
        let rows = vec![
            vec!["name".into(), "note".into()],
            vec!["Ada".into(), "a, \"quote\"\nline".into()],
        ];
        let encoded = serialize(&rows);
        assert_eq!(parse(&encoded).unwrap(), rows);
    }
    #[test]
    fn table_columns() {
        let table = Table::from_rows(parse("name,age\nAda,36").unwrap()).unwrap();
        assert_eq!(table.get(0, "age"), Some("36"));
    }
}
