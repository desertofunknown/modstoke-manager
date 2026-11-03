// Nexus Mods sign-in through OAuth 2.0 with PKCE, as Nexus requires for public applications.
// See https://modding.wiki/en/api/oauth2-guide.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};

const AUTHORIZE_URL: &str = "https://users.nexusmods.com/oauth/authorize";
const TOKEN_URL: &str = "https://users.nexusmods.com/oauth/token";
const REVOKE_URL: &str = "https://users.nexusmods.com/oauth/revoke";
// The registration Nexus Mods issued. The Manager is a public client: it proves each sign-in with
// PKCE and carries no client secret, which anyone could read out of the executable.
const CLIENT_ID: &str = "modstoke";
// Nexus sends the browser back to this local address, which the Manager listens on only while a
// sign-in is waiting. Browsers open it like any page, so no custom-scheme prompt can block it.
const LOOPBACK_ADDRESS: &str = "127.0.0.1:47823";
const REDIRECT_URI: &str = "http://127.0.0.1:47823/callback";
const SCOPE: &str = "openid public";
const CREDENTIAL_SERVICE: &str = "Modstoke Manager";
const REFRESH_TOKEN_ACCOUNT: &str = "nexusmods-refresh-token";
// Earlier builds stored a personal API key here. It is removed, never read.
const LEGACY_SERVICE: &str = "OpenACAI Mod Manager";
const LEGACY_KEY_ACCOUNT: &str = "nexusmods-api-key";
// How long a sign-in started in the browser may take to come back.
const SIGN_IN_WINDOW: Duration = Duration::from_secs(10 * 60);
// Tokens are refreshed this much early, so no request leaves with one about to expire.
const EXPIRY_MARGIN: Duration = Duration::from_secs(60);

struct PendingSignIn {
    state: String,
    verifier: String,
    started: Instant,
    sign_outs: u64,
}

// The refresh token is kept here as well as in Credential Manager, so a rotated token still works
// for this session if saving it fails.
struct Session {
    access_token: String,
    refresh_token: String,
    refresh_after: Instant,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

#[derive(Deserialize)]
struct OAuthError {
    error: Option<String>,
    error_description: Option<String>,
}

enum TokenFailure {
    // Nexus refused the code, the refresh token or the client, with its reason.
    Refused(String),
    Failed(String),
}

#[derive(Serialize)]
pub struct SignInStarted {
    url: String,
    opened: bool,
}

#[derive(Clone, Serialize)]
struct SignInFinished {
    error: Option<String>,
}

static PENDING: Mutex<Option<PendingSignIn>> = Mutex::new(None);
// Counts sign-outs, so a sign-in that finishes after one is thrown away.
static SIGN_OUTS: AtomicU64 = AtomicU64::new(0);
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
// Whether the loopback listener is running. Read and written only with PENDING locked.
static LISTENING: AtomicBool = AtomicBool::new(false);

// One lock covers the session and the saved refresh token, for sign-in, refresh and sign-out alike.
fn session() -> &'static tauri::async_runtime::Mutex<Option<Session>> {
    static SESSION: OnceLock<tauri::async_runtime::Mutex<Option<Session>>> = OnceLock::new();
    SESSION.get_or_init(|| tauri::async_runtime::Mutex::new(None))
}

// Lets the Nexus page hear when Nexus ends the sign-in, for example after the player revokes access.
pub fn remember_app(app: &tauri::AppHandle) {
    let _ = APP.set(app.clone());
}

// Opens the Nexus Mods sign-in page in the player's browser and starts listening for its return.
#[tauri::command]
pub fn nexus_begin_sign_in() -> Result<SignInStarted, String> {
    // RFC 7636 asks for a 43 to 128 character verifier; 32 random bytes encode to 43.
    let verifier = random_text(32)?;
    let state = random_text(16)?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let url = reqwest::Url::parse_with_params(
        AUTHORIZE_URL,
        [
            ("client_id", CLIENT_ID),
            ("response_type", "code"),
            ("scope", SCOPE),
            ("redirect_uri", REDIRECT_URI),
            ("state", state.as_str()),
            ("code_challenge_method", "S256"),
            ("code_challenge", challenge.as_str()),
        ],
    )
    .map_err(|error| error.to_string())?;
    let mut pending = PENDING.lock().map_err(|_| busy())?;
    *pending = Some(PendingSignIn {
        state,
        verifier,
        started: Instant::now(),
        sign_outs: SIGN_OUTS.load(Ordering::SeqCst),
    });
    if !LISTENING.load(Ordering::SeqCst) {
        let listener = match TcpListener::bind(LOOPBACK_ADDRESS).and_then(|listener| {
            listener.set_nonblocking(true)?;
            Ok(listener)
        }) {
            Ok(listener) => listener,
            Err(_) => {
                *pending = None;
                return Err("Another program is using the local port the Nexus Mods sign-in needs (47823). Close it and try again.".into());
            }
        };
        LISTENING.store(true, Ordering::SeqCst);
        std::thread::spawn(move || listen(listener));
    }
    drop(pending);
    let opened = crate::system::shell_execute("open", std::ffi::OsStr::new(url.as_str())).is_ok();
    Ok(SignInStarted { url: url.into(), opened })
}

// Serves the browser's return from Nexus until the waiting sign-in completes, is cancelled or lapses.
fn listen(listener: TcpListener) {
    loop {
        {
            let Ok(pending) = PENDING.lock() else { break };
            if !pending.as_ref().is_some_and(|waiting| waiting.started.elapsed() <= SIGN_IN_WINDOW) {
                // The port is released before another sign-in can try to bind it again.
                drop(listener);
                LISTENING.store(false, Ordering::SeqCst);
                return;
            }
        }
        match listener.accept() {
            // Browsers often open a spare connection they never use, so each one is read on its
            // own thread and cannot hold up the request that carries the callback.
            Ok((stream, _)) => {
                std::thread::spawn(move || answer(stream));
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => std::thread::sleep(Duration::from_millis(250)),
        }
    }
    LISTENING.store(false, Ordering::SeqCst);
}

// Handles one request to the loopback address. Only GET /callback with the waiting attempt's state
// finishes a sign-in; the page hears the outcome through the nexus-sign-in event.
fn answer(mut stream: TcpStream) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    while request.len() < 8192 && !request.windows(4).any(|end| end == b"\r\n\r\n") {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => request.extend_from_slice(&buffer[..read]),
        }
    }
    let request = String::from_utf8_lossy(&request);
    let target = request.lines().next()
        .and_then(|line| line.strip_prefix("GET "))
        .and_then(|rest| rest.split(' ').next())
        .unwrap_or_default();
    let callback = reqwest::Url::parse(&format!("http://127.0.0.1{target}")).ok()
        .filter(|url| target.starts_with('/') && url.path() == "/callback");
    let Some(callback) = callback else {
        respond(&mut stream, "404 Not Found", "Not found", "This address only finishes a Nexus Mods sign-in.");
        return;
    };
    let query: HashMap<String, String> = callback.query_pairs().into_owned().collect();
    let Some(pending) = take_pending(&query) else {
        respond(&mut stream, "400 Bad Request", "Sign-in link out of date",
            "This sign-in is no longer waiting. Start it again from Modstoke Manager.");
        return;
    };
    let result = tauri::async_runtime::block_on(complete_sign_in(pending, &query));
    match &result {
        Ok(()) => respond(&mut stream, "200 OK", "Signed in",
            "You're signed in to Nexus Mods. You can close this tab and go back to Modstoke Manager."),
        Err(error) => respond(&mut stream, "200 OK", "Sign-in failed", error),
    }
    if let Some(app) = APP.get() {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
        let _ = app.emit("nexus-sign-in", SignInFinished { error: result.err() });
    }
}

fn respond(stream: &mut TcpStream, status: &str, title: &str, message: &str) {
    let escape = |text: &str| text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let body = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Modstoke: {title}</title></head>\
         <body style=\"margin:0;padding:48px;background:#101217;color:#eef0f4;font:15px/1.6 system-ui,sans-serif\">\
         <h1 style=\"font-size:22px;margin:0 0 12px\">{title}</h1><p style=\"margin:0;color:#b8bfcc\">{}</p></body></html>",
        escape(message),
    );
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n{body}",
        body.len(),
    );
    let _ = stream.write_all(response.as_bytes());
}

// Only a callback carrying the waiting attempt's state counts. Anything else, such as a stale
// browser tab or a request from a web page, is ignored and the real attempt keeps waiting.
fn take_pending(query: &HashMap<String, String>) -> Option<PendingSignIn> {
    let mut pending = PENDING.lock().ok()?;
    if query.get("state") != Some(&pending.as_ref()?.state) {
        return None;
    }
    pending.take()
}

async fn complete_sign_in(pending: PendingSignIn, query: &HashMap<String, String>) -> Result<(), String> {
    if pending.started.elapsed() > SIGN_IN_WINDOW {
        return Err("The sign-in took too long. Start it again from the Manager.".into());
    }
    if let Some(error) = query.get("error") {
        return Err(if error == "access_denied" {
            "Sign-in was cancelled on Nexus Mods.".into()
        } else {
            format!("Nexus Mods refused the sign-in ({error}).")
        });
    }
    let code = query.get("code").filter(|code| !code.is_empty())
        .ok_or("Nexus Mods did not return a sign-in code.")?;
    let tokens = request_tokens(&[
        ("grant_type", "authorization_code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", REDIRECT_URI),
        ("code", code),
        ("code_verifier", &pending.verifier),
    ])
    .await
    .map_err(|failure| match failure {
        TokenFailure::Refused(reason) => format!("Nexus Mods did not accept the sign-in: {reason}"),
        TokenFailure::Failed(error) => error,
    })?;
    let refresh_token = tokens.refresh_token.clone().ok_or("Nexus Mods did not return a refresh token.")?;

    let mut session = session().lock().await;
    if SIGN_OUTS.load(Ordering::SeqCst) != pending.sign_outs {
        tauri::async_runtime::spawn(revoke(refresh_token));
        return Err("You signed out before the sign-in finished.".into());
    }
    let replaced = match session.as_ref() {
        Some(current) => Some(current.refresh_token.clone()),
        None => read_refresh_token().ok().flatten(),
    };
    refresh_token_entry()?.set_password(&refresh_token).map_err(credential_error)?;
    *session = Some(Session::new(&tokens, refresh_token.clone()));
    drop(session);
    // Signing in again, possibly as someone else, retires the token it replaces.
    if let Some(replaced) = replaced.filter(|replaced| *replaced != refresh_token) {
        tauri::async_runtime::spawn(revoke(replaced));
    }
    Ok(())
}

// The current access token, refreshed first when it is close to expiry. None means signed out.
pub(crate) async fn access_token() -> Result<Option<String>, String> {
    let mut session = session().lock().await;
    if let Some(current) = session.as_ref().filter(|current| Instant::now() < current.refresh_after) {
        return Ok(Some(current.access_token.clone()));
    }
    let refresh_token = match session.as_ref() {
        Some(current) => current.refresh_token.clone(),
        None => match read_refresh_token()? {
            Some(token) => token,
            None => return Ok(None),
        },
    };
    match request_tokens(&[
        ("grant_type", "refresh_token"),
        ("client_id", CLIENT_ID),
        ("refresh_token", &refresh_token),
    ])
    .await
    {
        Ok(tokens) => {
            let next = tokens.refresh_token.clone().unwrap_or_else(|| refresh_token.clone());
            if next != refresh_token {
                // If this fails, the rotated token still works from memory until the Manager closes.
                let _ = refresh_token_entry().and_then(|entry| entry.set_password(&next).map_err(credential_error));
            }
            let current = Session::new(&tokens, next);
            let value = current.access_token.clone();
            *session = Some(current);
            Ok(Some(value))
        }
        // Nexus refuses the refresh token once the player revokes the Manager's access.
        Err(TokenFailure::Refused(_)) => {
            *session = None;
            delete_refresh_token()?;
            drop(session);
            if let Some(app) = APP.get() {
                let _ = app.emit("nexus-signed-out", ());
            }
            Ok(None)
        }
        Err(TokenFailure::Failed(error)) => Err(error),
    }
}

// After Nexus rejects an access token, the next request refreshes it instead of reusing it.
pub(crate) async fn forget_access_token() {
    if let Some(current) = session().lock().await.as_mut() {
        current.refresh_after = Instant::now();
    }
}

#[tauri::command]
pub async fn nexus_sign_out() -> Result<(), String> {
    SIGN_OUTS.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut pending) = PENDING.lock() {
        *pending = None;
    }
    let mut session = session().lock().await;
    let stored = read_refresh_token()?;
    let refresh_token = session.take().map(|current| current.refresh_token).or(stored);
    delete_refresh_token()?;
    drop(session);
    // Revoking is a courtesy to Nexus; the token is already gone from this machine.
    if let Some(token) = refresh_token {
        tauri::async_runtime::spawn(revoke(token));
    }
    Ok(())
}

// Personal API keys are no longer accepted, so one left by an earlier build is deleted.
pub(crate) fn forget_legacy_api_key() {
    if let Ok(entry) = keyring::Entry::new(LEGACY_SERVICE, LEGACY_KEY_ACCOUNT) {
        let _ = entry.delete_password();
    }
}

impl Session {
    fn new(tokens: &TokenResponse, refresh_token: String) -> Self {
        let lifetime = Duration::from_secs(tokens.expires_in.unwrap_or(3600));
        Self {
            access_token: tokens.access_token.clone(),
            refresh_token,
            refresh_after: Instant::now() + lifetime.saturating_sub(EXPIRY_MARGIN),
        }
    }
}

async fn request_tokens(form: &[(&str, &str)]) -> Result<TokenResponse, TokenFailure> {
    let client = client().map_err(TokenFailure::Failed)?;
    let response = client.post(TOKEN_URL).form(form).send().await
        .map_err(|error| TokenFailure::Failed(format!("Could not reach Nexus Mods: {error}")))?;
    let status = response.status();
    if status.is_client_error() {
        let refusal = response.json::<OAuthError>().await.ok();
        let code = refusal.as_ref().and_then(|refusal| refusal.error.clone()).unwrap_or_default();
        let reason = refusal
            .and_then(|refusal| refusal.error_description.or(refusal.error))
            .unwrap_or_else(|| status.to_string());
        // Only an OAuth refusal ends a sign-in. Throttling, timeouts and proxy pages are worth retrying.
        return Err(if matches!(code.as_str(), "invalid_grant" | "invalid_client" | "unauthorized_client") {
            TokenFailure::Refused(reason)
        } else {
            TokenFailure::Failed(format!("Nexus Mods sign-in failed: {reason}"))
        });
    }
    if !status.is_success() {
        return Err(TokenFailure::Failed(format!("Nexus Mods sign-in failed: {status}")));
    }
    response.json::<TokenResponse>().await
        .map_err(|_| TokenFailure::Failed("Nexus Mods returned an unreadable sign-in response.".into()))
}

async fn revoke(token: String) {
    if let Ok(client) = client() {
        let _ = client
            .post(REVOKE_URL)
            .form(&[("token", token.as_str()), ("client_id", CLIENT_ID), ("token_type_hint", "refresh_token")])
            .send()
            .await;
    }
}

fn client() -> Result<reqwest::Client, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Application-Name", reqwest::header::HeaderValue::from_static(crate::nexus::NEXUS_APPLICATION_NAME));
    headers.insert("Application-Version", reqwest::header::HeaderValue::from_static(crate::nexus::NEXUS_APPLICATION_VERSION));
    reqwest::Client::builder()
        .user_agent(crate::nexus::NEXUS_USER_AGENT)
        .default_headers(headers)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not initialize the Nexus Mods connection.".to_string())
}

fn random_text(bytes: usize) -> Result<String, String> {
    let mut buffer = vec![0u8; bytes];
    getrandom::getrandom(&mut buffer).map_err(|_| "Could not start the sign-in securely.".to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(buffer))
}

fn refresh_token_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(CREDENTIAL_SERVICE, REFRESH_TOKEN_ACCOUNT).map_err(credential_error)
}

fn read_refresh_token() -> Result<Option<String>, String> {
    match refresh_token_entry()?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(credential_error(error)),
    }
}

fn delete_refresh_token() -> Result<(), String> {
    match refresh_token_entry()?.delete_password() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(credential_error(error)),
    }
}

fn credential_error(error: keyring::Error) -> String {
    format!("Could not use the Windows credential store for the Nexus Mods sign-in: {error}")
}

fn busy() -> String {
    "The Nexus Mods sign-in is busy. Try again.".into()
}
