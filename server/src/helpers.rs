//! Functions useful in the server

use actix_web::http::header::HeaderMap;
use atomic_lib::authentication::AuthValues;

use crate::{appstate::AppState, content_types::ContentType, errors::AtomicServerResult};

// Returns None if the string is empty.
// Useful for parsing form inputs.
pub fn empty_to_nothing(string: Option<String>) -> Option<String> {
    match string.as_ref() {
        Some(st) => {
            if st.is_empty() {
                None
            } else {
                string
            }
        }
        None => None,
    }
}

/// Returns the authentication headers from the request
#[tracing::instrument(skip_all)]
pub fn get_auth_headers(
    map: &HeaderMap,
    requested_subject: String,
) -> AtomicServerResult<Option<AuthValues>> {
    let public_key = map.get("x-atomic-public-key");
    let signature = map.get("x-atomic-signature");
    let timestamp = map.get("x-atomic-timestamp");
    let agent = map.get("x-atomic-agent");
    match (public_key, signature, timestamp, agent) {
        (Some(pk), Some(sig), Some(ts), Some(a)) => Ok(Some(AuthValues {
            public_key: pk
                .to_str()
                .map_err(|_e| "Only string headers allowed")?
                .to_string(),
            signature: sig
                .to_str()
                .map_err(|_e| "Only string headers allowed")?
                .to_string(),
            agent_subject: a
                .to_str()
                .map_err(|_e| "Only string headers allowed")?
                .to_string(),
            timestamp: ts
                .to_str()
                .map_err(|_e| "Only string headers allowed")?
                .parse::<i64>()
                .map_err(|_e| "Timestamp must be a number (milliseconds since unix epoch)")?,
            requested_subject,
        })),
        (None, None, None, None) => Ok(None),
        _missing => Err("Missing authentication headers. You need `x-atomic-public-key`, `x-atomic-signature`, `x-atomic-agent` and `x-atomic-timestamp` for authentication checks.".into()),
    }
}

/// Checks for authentication headers and returns Some agent's subject if everything is well.
/// Skips these checks in public_mode and returns Ok(None).
#[tracing::instrument(skip(appstate))]
pub fn get_client_agent(
    headers: &HeaderMap,
    appstate: &AppState,
    requested_subject: String,
) -> AtomicServerResult<Option<String>> {
    if appstate.config.opts.public_mode {
        return Ok(None);
    }
    // Authentication check. If the user has no headers, continue with the Public Agent.
    let auth_header_values = get_auth_headers(headers, requested_subject)?;
    let for_agent = atomic_lib::authentication::get_agent_from_headers_and_check(
        auth_header_values,
        &appstate.store,
    )
    .map_err(|e| format!("Authentication failed: {}", e))?;
    Ok(Some(for_agent))
}

/// Finds the extension
pub fn try_extension(path: &str) -> Option<(ContentType, &str)> {
    let items: Vec<&str> = path.split('.').collect();
    if items.len() == 2 {
        let path = items[0];
        let content_type = match items[1] {
            "json" => ContentType::Json,
            "jsonld" => ContentType::JsonLd,
            "jsonad" => ContentType::JsonAd,
            "html" => ContentType::Html,
            "ttl" => ContentType::Turtle,
            _ => return None,
        };
        return Some((content_type, path));
    }
    None
}
