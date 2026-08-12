//! SMTP email sending (`std::email::*`) via the `lettre` crate + rustls.
//!
//! Blocking API surface for `.titan`:
//!
//! * `send_simple(host, port, user, pass, from, to, subject, body)` — plain text
//! * `send_html(host, port, user, pass, from, to, subject, text, html)` — multipart
//! * `send_bytes_attachment(...)` — same as `send_html` plus one binary attachment
//!
//! Every helper opens a fresh SMTP connection with STARTTLS if the port is 587,
//! or implicit TLS if the port is 465. Port 25 is only used when explicitly requested.

use lettre::message::{header::ContentType, Attachment, Body, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmailError {
    #[error("email build error: {0}")]
    Build(String),
    #[error("SMTP transport error: {0}")]
    Transport(String),
}

fn to_build(error: impl std::fmt::Display) -> EmailError {
    EmailError::Build(error.to_string())
}
fn to_transport(error: impl std::fmt::Display) -> EmailError {
    EmailError::Transport(error.to_string())
}

fn make_transport(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
) -> Result<SmtpTransport, EmailError> {
    let credentials = Credentials::new(user.into(), pass.into());
    let builder = match port {
        465 => SmtpTransport::relay(host).map_err(to_transport)?.port(465),
        587 => SmtpTransport::starttls_relay(host)
            .map_err(to_transport)?
            .port(587),
        25 => SmtpTransport::builder_dangerous(host).port(25),
        other => SmtpTransport::starttls_relay(host)
            .map_err(to_transport)?
            .port(other),
    };
    Ok(builder.credentials(credentials).build())
}

/// Send a plain-text email.
pub fn send_simple(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
    from: &str,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<String, EmailError> {
    let email = Message::builder()
        .from(from.parse().map_err(to_build)?)
        .to(to.parse().map_err(to_build)?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body.to_string())
        .map_err(to_build)?;
    let mailer = make_transport(host, port, user, pass)?;
    let response = mailer.send(&email).map_err(to_transport)?;
    Ok(format!(
        "code={} messages={:?}",
        response.code(),
        response.message().collect::<Vec<_>>()
    ))
}

/// Send an HTML + plain-text alternative email.
pub fn send_html(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
    from: &str,
    to: &str,
    subject: &str,
    text: &str,
    html: &str,
) -> Result<String, EmailError> {
    let email = Message::builder()
        .from(from.parse().map_err(to_build)?)
        .to(to.parse().map_err(to_build)?)
        .subject(subject)
        .multipart(MultiPart::alternative_plain_html(
            text.to_string(),
            html.to_string(),
        ))
        .map_err(to_build)?;
    let mailer = make_transport(host, port, user, pass)?;
    let response = mailer.send(&email).map_err(to_transport)?;
    Ok(format!(
        "code={} messages={:?}",
        response.code(),
        response.message().collect::<Vec<_>>()
    ))
}

/// Send an HTML email with one binary attachment (`bytes` with the given `filename` and `mime`).
pub fn send_with_attachment(
    host: &str,
    port: u16,
    user: &str,
    pass: &str,
    from: &str,
    to: &str,
    subject: &str,
    html: &str,
    filename: &str,
    mime: &str,
    bytes: &[u8],
) -> Result<String, EmailError> {
    let content_type: ContentType = mime.parse().map_err(to_build)?;
    let attachment =
        Attachment::new(filename.to_string()).body(Body::new(bytes.to_vec()), content_type);
    let body = MultiPart::mixed()
        .singlepart(SinglePart::html(html.to_string()))
        .singlepart(attachment);
    let email = Message::builder()
        .from(from.parse().map_err(to_build)?)
        .to(to.parse().map_err(to_build)?)
        .subject(subject)
        .multipart(body)
        .map_err(to_build)?;
    let mailer = make_transport(host, port, user, pass)?;
    let response = mailer.send(&email).map_err(to_transport)?;
    Ok(format!(
        "code={} messages={:?}",
        response.code(),
        response.message().collect::<Vec<_>>()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_email_addresses() {
        // Even without a network, an invalid address must be caught by the builder.
        let error = send_simple(
            "smtp.example.com",
            587,
            "u",
            "p",
            "not an email",
            "also invalid",
            "hi",
            "hi",
        );
        assert!(error.is_err(), "expected a build error, got {error:?}");
    }

    /// Full round-trip test is opt-in; needs a reachable SMTP account.
    /// Set env: TITAN_SMTP_HOST, TITAN_SMTP_PORT, TITAN_SMTP_USER, TITAN_SMTP_PASS,
    /// TITAN_SMTP_FROM, TITAN_SMTP_TO.
    #[test]
    fn live_send_when_configured() {
        let host = std::env::var("TITAN_SMTP_HOST");
        let port = std::env::var("TITAN_SMTP_PORT");
        if host.is_err() || port.is_err() {
            return;
        }
        let host = host.unwrap();
        let port: u16 = port.unwrap().parse().unwrap();
        let user = std::env::var("TITAN_SMTP_USER").unwrap();
        let pass = std::env::var("TITAN_SMTP_PASS").unwrap();
        let from = std::env::var("TITAN_SMTP_FROM").unwrap();
        let to = std::env::var("TITAN_SMTP_TO").unwrap();
        let out = send_simple(
            &host,
            port,
            &user,
            &pass,
            &from,
            &to,
            "TITAN test",
            "Hola, este email viene del test de titan_stdlib.",
        )
        .unwrap();
        assert!(out.starts_with("code="));
    }
}
