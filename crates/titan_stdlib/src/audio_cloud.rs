//! Fase 43 — nube musical en Telegram.
//!
//! Tus canciones viven en un canal privado de Telegram («Mi Música») y
//! Titan las toca directo de la nube, igual que Spotify o YouTube: solo
//! viajan unos segundos de audio por adelantado (unos pocos MB en
//! memoria) y nada se queda guardado en el teléfono, salvo que TÚ pidas
//! descargar una canción con [`download`].
//!
//! Cómo se conecta (una sola vez):
//! 1. Crea tu app en <https://my.telegram.org> (2 minutos, gratis) y
//!    anota el `api_id` y el `api_hash`.
//! 2. `cloud_setup(api_id, api_hash)` los guarda en `~/.titan/cloud.json`.
//! 3. `cloud_login_phone("+34...")` → Telegram te manda un código.
//! 4. `cloud_login_code("12345")` → listo (si tienes clave de dos pasos,
//!    `cloud_login_password("...")`).
//!
//! La sesión queda guardada en `~/.titan/telegram.session.json`, así que
//! las próximas veces entras sin código. `cloud_logout()` borra la
//! sesión (tus canciones siguen intactas en Telegram).
//!
//! Streaming honesto: cada pista nube se abre como `cloud:<id>` y el
//! motor (Fase 42A) la decodifica sobre la marcha con symphonia. Buscar
//! (seek) re-pide desde el nuevo punto; la caché por pista nunca pasa
//! de unos pocos MB.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use grammers_client::media::{Attribute, Downloadable, Media};
use grammers_client::message::InputMessage;
use grammers_client::client::{LoginToken, PasswordToken};
use grammers_client::{Client, SignInError};
use grammers_mtsender::{SenderPool, SenderPoolFatHandle};
use grammers_session::types::{
    ChannelState, DcOption, PeerId, PeerInfo, PeerKind, PeerRef, UpdateState, UpdatesState,
};
use grammers_session::{BoxFuture, Session, SessionData};
use serde::{Deserialize, Serialize};
use symphonia::core::io::MediaSource;
use thiserror::Error;
use tokio::runtime::{Builder, Handle, Runtime};

/// Nombre del canal privado donde viven las canciones.
pub const CHANNEL_TITLE: &str = "Mi Música";
/// Prefijo de las pistas nube dentro del motor (`cloud:<id de mensaje>`).
pub const CLOUD_PREFIX: &str = "cloud:";
/// Pedazos de descarga: 512 KiB alineados (límite de Telegram).
const FETCH_CHUNK: usize = 512 * 1024;
/// Cuánto pide de una vez el lector de streaming.
const READ_AHEAD: usize = 256 * 1024;

// ------------------------------------------------------------------ errores

/// Todo lo que puede salir mal hablando con Telegram, en criollo.
#[derive(Debug, Error)]
pub enum CloudError {
    #[error("falta configurar: {0}")]
    Setup(String),
    #[error("login: {0}")]
    Auth(String),
    #[error("red/telegram: {0}")]
    Net(String),
    #[error("canal: {0}")]
    Channel(String),
    #[error("pista: {0}")]
    Track(String),
    #[error("archivo: {0}")]
    Io(String),
    #[error("audio: {0}")]
    Decode(String),
    #[error("motor: {0}")]
    Engine(String),
}

impl From<std::io::Error> for CloudError {
    fn from(e: std::io::Error) -> Self {
        CloudError::Io(e.to_string())
    }
}

// ------------------------------------------------- rutas y configuración

fn home_dir() -> Result<PathBuf, CloudError> {
    // `dirs` es opcional en este crate; HOME basta (Termux lo define).
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| CloudError::Setup("no se encontró la carpeta HOME".to_string()))
}

fn titan_dir() -> Result<PathBuf, CloudError> {
    Ok(home_dir()?.join(".titan"))
}

fn config_path() -> Result<PathBuf, CloudError> {
    Ok(titan_dir()?.join("cloud.json"))
}

fn session_path() -> Result<PathBuf, CloudError> {
    Ok(titan_dir()?.join("telegram.session.json"))
}

/// Credenciales de app + recuerdos (dueño, canal). La sesión MTProto
/// (claves) vive aparte en `telegram.session.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CloudConfig {
    #[serde(default)]
    api_id: i32,
    #[serde(default)]
    api_hash: String,
    #[serde(default)]
    phone: String,
    #[serde(default)]
    user_name: String,
    #[serde(default)]
    channel_id: Option<i64>,
}

fn load_config() -> CloudConfig {
    config_path()
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save_config(cfg: &CloudConfig) -> Result<(), CloudError> {
    let dir = titan_dir()?;
    std::fs::create_dir_all(&dir)?;
    let bytes = serde_json::to_vec_pretty(cfg)
        .map_err(|e| CloudError::Io(format!("config JSON: {e}")))?;
    std::fs::write(config_path()?, bytes)?;
    Ok(())
}

// ------------------------------------------------------- sesión en JSON

/// `SessionData` tal cual en disco. Los mapas van como `Vec` de pares
/// porque JSON no sabe deserializar claves que no son texto.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionFile {
    home_dc: i32,
    dc_options: Vec<(i32, DcOption)>,
    peer_infos: Vec<(PeerId, PeerInfo)>,
    updates_state: UpdatesState,
}

impl From<&SessionData> for SessionFile {
    fn from(data: &SessionData) -> Self {
        SessionFile {
            home_dc: data.home_dc,
            dc_options: data
                .dc_options
                .iter()
                .map(|(k, v)| (*k, v.clone()))
                .collect(),
            peer_infos: data
                .peer_infos
                .iter()
                .map(|(k, v)| (*k, v.clone()))
                .collect(),
            updates_state: data.updates_state.clone(),
        }
    }
}

impl From<SessionFile> for SessionData {
    fn from(file: SessionFile) -> Self {
        SessionData {
            home_dc: file.home_dc,
            dc_options: file.dc_options.into_iter().collect(),
            peer_infos: file.peer_infos.into_iter().collect(),
            updates_state: file.updates_state,
        }
    }
}

#[derive(Debug)]
pub enum JsonSessionError {
    Poisoned,
}

impl fmt::Display for JsonSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonSessionError::Poisoned => write!(f, "session lock is poisoned"),
        }
    }
}

impl std::error::Error for JsonSessionError {}

/// Sesión MTProto con espejo en memoria + guardado automático a JSON.
///
/// Es el `MemorySession` de grammers con persistencia: cada cambio
/// (claves, canal home, contactos, estado) se escribe al archivo, así
/// el login sobrevive entre ejecuciones sin SQLite ni C compilado.
pub struct JsonFileSession {
    path: PathBuf,
    data: Mutex<SessionData>,
}

impl JsonFileSession {
    pub fn open(path: PathBuf) -> Self {
        let data = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<SessionFile>(&b).ok())
            .map(SessionData::from)
            .unwrap_or_default();
        JsonFileSession {
            path,
            data: Mutex::new(data),
        }
    }

    fn data(&self) -> Result<std::sync::MutexGuard<'_, SessionData>, JsonSessionError> {
        self.data.lock().map_err(|_| JsonSessionError::Poisoned)
    }

    /// Guarda lo mejor posible; si falla, la sesión en memoria sigue
    /// valiendo para esta ejecución (el error se ignora a propósito:
    /// un disco lleno no debe tumbar la reproducción).
    fn persist(&self) {
        let snapshot = match self.data() {
            Ok(guard) => SessionFile::from(&*guard),
            Err(_) => return,
        };
        let bytes = match serde_json::to_vec(&snapshot) {
            Ok(bytes) => bytes,
            Err(_) => return,
        };
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&self.path, bytes);
    }
}

impl Session for JsonFileSession {
    type Error = JsonSessionError;

    fn home_dc_id(&self) -> Result<i32, JsonSessionError> {
        Ok(self.data()?.home_dc)
    }

    fn set_home_dc_id(&self, dc_id: i32) -> BoxFuture<'_, Result<(), JsonSessionError>> {
        Box::pin(async move {
            self.data()?.home_dc = dc_id;
            self.persist();
            Ok(())
        })
    }

    fn dc_option(&self, dc_id: i32) -> Result<Option<DcOption>, JsonSessionError> {
        Ok(self.data()?.dc_options.get(&dc_id).cloned())
    }

    fn set_dc_option(&self, dc_option: &DcOption) -> BoxFuture<'_, Result<(), JsonSessionError>> {
        let dc_option = dc_option.clone();
        Box::pin(async move {
            self.data()?
                .dc_options
                .insert(dc_option.id, dc_option.clone());
            self.persist();
            Ok(())
        })
    }

    fn peer(&self, peer: PeerId) -> BoxFuture<'_, Result<Option<PeerInfo>, JsonSessionError>> {
        Box::pin(async move { Ok(self.data()?.peer_infos.get(&peer).cloned()) })
    }

    fn cache_peer(&self, peer: PeerInfo) -> BoxFuture<'_, Result<(), JsonSessionError>> {
        Box::pin(async move {
            self.data()?
                .peer_infos
                .entry(peer.id())
                .or_insert_with(|| peer.clone())
                .extend_info(&peer);
            self.persist();
            Ok(())
        })
    }

    fn updates_state(&self) -> BoxFuture<'_, Result<UpdatesState, JsonSessionError>> {
        Box::pin(async move { Ok(self.data()?.updates_state.clone()) })
    }

    fn set_update_state(
        &self,
        update: UpdateState,
    ) -> BoxFuture<'_, Result<(), JsonSessionError>> {
        Box::pin(async move {
            let mut data = self.data()?;
            match update {
                UpdateState::All(updates_state) => {
                    data.updates_state = updates_state;
                }
                UpdateState::Primary { pts, date, seq } => {
                    data.updates_state.pts = pts;
                    data.updates_state.date = date;
                    data.updates_state.seq = seq;
                }
                UpdateState::Secondary { qts } => {
                    data.updates_state.qts = qts;
                }
                UpdateState::Channel { id, pts } => {
                    data.updates_state.channels.retain(|c| c.id != id);
                    data.updates_state.channels.push(ChannelState { id, pts });
                }
            }
            drop(data);
            self.persist();
            Ok(())
        })
    }
}

// ------------------------------------------------------------- estado

/// Una pista nube resuelta (caché en memoria tras [`library`]).
#[derive(Debug, Clone)]
pub struct CloudRef {
    pub msg_id: i32,
    pub title: String,
    pub artist: String,
    pub duration_secs: f64,
    pub size: u64,
    pub file: String,
    pub mime: String,
    media: Media,
}

struct CloudState {
    rt: Option<Runtime>,
    session: Option<Arc<JsonFileSession>>,
    /// Mantiene vivo al pool (si se suelta, se cae la conexión).
    pool: Option<SenderPoolFatHandle>,
    client: Option<Client>,
    login: Option<LoginToken>,
    password: Option<PasswordToken>,
    channel: Option<PeerRef>,
    refs: HashMap<i32, CloudRef>,
}

impl CloudState {
    const fn new() -> Self {
        CloudState {
            rt: None,
            session: None,
            pool: None,
            client: None,
            login: None,
            password: None,
            channel: None,
            refs: HashMap::new(),
        }
    }
}

fn slot() -> &'static Mutex<CloudState> {
    static SLOT: std::sync::OnceLock<Mutex<CloudState>> = std::sync::OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(CloudState::new()))
}

fn lock_slot() -> std::sync::MutexGuard<'static, CloudState> {
    slot().lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Corre un futuro con tope de tiempo, desde código normal (no-async).
fn run<T, F>(handle: &Handle, secs: u64, fut: F) -> Result<T, CloudError>
where
    F: std::future::Future<Output = Result<T, CloudError>>,
{
    handle
        .block_on(async move {
            match tokio::time::timeout(Duration::from_secs(secs), fut).await {
                Ok(inner) => inner,
                Err(_) => Err(CloudError::Net(format!(
                    "Telegram no respondió en {secs} s (¿hay internet?)"
                ))),
            }
        })
}

/// Runtime compartido (hilos propios; se puede llamar desde el hilo
/// de audio y desde la VM a la vez).
fn rt_handle() -> Result<Handle, CloudError> {
    let mut guard = lock_slot();
    if guard.rt.is_none() {
        let rt = Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("titan-tg")
            .enable_all()
            .build()
            .map_err(|e| CloudError::Net(format!("no arranca el runtime: {e}")))?;
        guard.rt = Some(rt);
    }
    Ok(guard
        .rt
        .as_ref()
        .map(|rt| rt.handle().clone())
        .ok_or_else(|| CloudError::Net("sin runtime".to_string()))?)
}

/// Conecta (o reutiliza la conexión) y entrega cliente listo.
fn ensure_client() -> Result<(Handle, Client), CloudError> {
    ensure_hooks();
    let cfg = load_config();
    if cfg.api_id == 0 || cfg.api_hash.is_empty() {
        return Err(CloudError::Setup(
            "falta cloud_setup(api_id, api_hash): crea tu app en https://my.telegram.org".to_string(),
        ));
    }
    let handle = rt_handle()?;
    let mut guard = lock_slot();
    if let Some(client) = guard.client.clone() {
        return Ok((handle, client));
    }
    let session = Arc::new(JsonFileSession::open(
        session_path().unwrap_or_else(|_| PathBuf::from("telegram.session.json")),
    ));
    let pool = SenderPool::new(Arc::clone(&session), cfg.api_id);
    let SenderPool {
        runner, handle: pool_handle, ..
    } = pool;
    // La tarea queda andando sola (detached) hasta que se suelte el pool.
    handle.spawn(runner.run());
    let client = Client::new(pool_handle.clone());
    guard.session = Some(session);
    guard.pool = Some(pool_handle);
    guard.client = Some(client.clone());
    Ok((handle, client))
}

// ------------------------------------------------------------------ login

/// Guarda el `api_id` + `api_hash` de tu app (<https://my.telegram.org>).
pub fn setup(api_id: i64, api_hash: &str) -> Result<String, CloudError> {
    ensure_hooks();
    let api_id: i32 = api_id
        .try_into()
        .map_err(|_| CloudError::Setup("api_id fuera de rango".to_string()))?;
    if api_id <= 0 || api_hash.trim().is_empty() {
        return Err(CloudError::Setup(
            "api_id y api_hash no pueden estar vacíos".to_string(),
        ));
    }
    let mut cfg = load_config();
    // Credenciales nuevas = sesión vieja inválida: se suelta todo.
    if cfg.api_id != api_id || cfg.api_hash != api_hash.trim() {
        logout_quiet();
    }
    cfg.api_id = api_id;
    cfg.api_hash = api_hash.trim().to_string();
    save_config(&cfg)?;
    Ok("nube configurada; sigue con cloud_login_phone(\"+...\")".to_string())
}

/// Pide el código de login al número dado (formato internacional).
pub fn login_phone(phone: &str) -> Result<String, CloudError> {
    let phone = phone.trim();
    if phone.len() < 6 || !phone.starts_with('+') {
        return Err(CloudError::Auth(
            "el número va en formato internacional, ej. \"+34600111222\"".to_string(),
        ));
    }
    let (handle, client) = ensure_client()?;
    let cfg = load_config();
    let authorized =
        run(&handle, 30, async { Ok(client.is_authorized().await.map_err(rpc_err)?) })?;
    if authorized {
        let mut cfg = cfg;
        cfg.phone = phone.to_string();
        save_config(&cfg).ok();
        let who = who_name(&cfg);
        return Ok(format!("already {who}"));
    }
    let token = run(&handle, 60, async {
        client
            .request_login_code(phone, &cfg.api_hash)
            .await
            .map_err(|e| CloudError::Auth(format!("Telegram rechazó el número ({e})")))
    })?;
    lock_slot().login = Some(token);
    let mut cfg = cfg;
    cfg.phone = phone.to_string();
    save_config(&cfg).ok();
    Ok(format!(
        "code_sent revisa Telegram en {phone} y pasa el código a cloud_login_code"
    ))
}

/// Completa el login con el código que llegó a Telegram.
pub fn login_code(code: &str) -> Result<String, CloudError> {
    let code = code.trim().replace(' ', "");
    if code.is_empty() {
        return Err(CloudError::Auth(
            "el código no puede estar vacío".to_string(),
        ));
    }
    let token = lock_slot().login.take().ok_or_else(|| {
        CloudError::Auth("primero pide el código con cloud_login_phone".to_string())
    })?;
    let (handle, client) = ensure_client()?;
    let result = run(&handle, 60, async {
        Ok(client.sign_in(&token, &code).await)
    })?;
    match result {
        Ok(user) => {
            let name = user.first_name().unwrap_or_default().trim().to_string();
            let mut cfg = load_config();
            cfg.user_name = if name.is_empty() {
                "yo".to_string()
            } else {
                name.clone()
            };
            save_config(&cfg).ok();
            Ok(format!("user {name}"))
        }
        Err(SignInError::PasswordRequired(password_token)) => {
            let hint = password_token.hint().unwrap_or_default().to_string();
            lock_slot().password = Some(password_token);
            if hint.is_empty() {
                Ok("password_required (tienes verificación en dos pasos)".to_string())
            } else {
                Ok(format!("password_required (pista: {hint})"))
            }
        }
        Err(SignInError::SignUpRequired { .. }) => Err(CloudError::Auth(
            "ese número no tiene cuenta de Telegram; créala primero en la app".to_string(),
        )),
        Err(e) => Err(CloudError::Auth(format!("código inválido ({e})"))),
    }
}

/// Segundo factor (solo si `cloud_login_code` pidió contraseña).
pub fn login_password(password: &str) -> Result<String, CloudError> {
    if password.is_empty() {
        return Err(CloudError::Auth("la contraseña no puede estar vacía".to_string()));
    }
    let token = lock_slot().password.take().ok_or_else(|| {
        CloudError::Auth("no hay login esperando contraseña".to_string())
    })?;
    let (handle, client) = ensure_client()?;
    let user = run(&handle, 60, async {
        client
            .check_password(token, password)
            .await
            .map_err(|e| CloudError::Auth(format!("contraseña inválida ({e})")))
    })?;
    let name = user.first_name().unwrap_or_default().trim().to_string();
    let mut cfg = load_config();
    cfg.user_name = if name.is_empty() {
        "yo".to_string()
    } else {
        name.clone()
    };
    save_config(&cfg).ok();
    Ok(format!("user {name}"))
}

fn who_name(cfg: &CloudConfig) -> String {
    if cfg.user_name.is_empty() {
        "sesión guardada".to_string()
    } else {
        cfg.user_name.clone()
    }
}

fn rpc_err(e: impl fmt::Display) -> CloudError {
    CloudError::Net(e.to_string())
}

/// Foto del estado: nunca falla (los errores van en `note`).
#[derive(Debug, Clone)]
pub struct CloudStatus {
    pub configured: bool,
    pub connected: bool,
    pub authorized: bool,
    pub user: String,
    pub phone: String,
    pub channel: String,
    pub session_file: bool,
    pub cached_tracks: usize,
    pub note: String,
}

pub fn status() -> CloudStatus {
    ensure_hooks();
    let cfg = load_config();
    let configured = cfg.api_id != 0 && !cfg.api_hash.is_empty();
    let session_file = session_path().map(|p| p.is_file()).unwrap_or(false);
    let (connected, channel, cached) = {
        let guard = lock_slot();
        (
            guard.client.is_some(),
            guard.channel.is_some(),
            guard.refs.len(),
        )
    };
    let mut st = CloudStatus {
        configured,
        connected,
        authorized: false,
        user: who_name(&cfg),
        phone: cfg.phone.clone(),
        channel: if channel {
            CHANNEL_TITLE.to_string()
        } else {
            String::new()
        },
        session_file,
        cached_tracks: cached,
        note: String::new(),
    };
    if !configured {
        st.note = "falta cloud_setup(api_id, api_hash)".to_string();
        return st;
    }
    if !connected {
        st.note = "sin conectar (conecta con cloud_login_phone)".to_string();
        return st;
    }
    // Conectado: ¿la sesión sigue autorizada? (barato, sin red extra
    // si la clave ya está validada).
    let got = (|| -> Result<bool, CloudError> {
        let (handle, client) = ensure_client()?;
        run(&handle, 30, async {
            Ok(client.is_authorized().await.map_err(rpc_err)?)
        })
    })();
    match got {
        Ok(ok) => {
            st.authorized = ok;
            if !ok {
                st.note = "conectado pero sin login (cloud_login_phone)".to_string();
            }
        }
        Err(e) => st.note = e.to_string(),
    }
    st
}

/// Suelta la conexión y borra la sesión guardada (las canciones en
/// Telegram no se tocan; las credenciales de app se conservan).
pub fn logout() -> Result<String, CloudError> {
    logout_quiet();
    if let Ok(path) = session_path() {
        let _ = std::fs::remove_file(path);
    }
    let mut cfg = load_config();
    cfg.user_name.clear();
    cfg.channel_id = None;
    save_config(&cfg).ok();
    Ok("sesión cerrada; tus canciones siguen en Telegram".to_string())
}

fn logout_quiet() {
    let mut guard = lock_slot();
    guard.client = None;
    guard.pool = None;
    guard.session = None;
    guard.login = None;
    guard.password = None;
    guard.channel = None;
    guard.refs.clear();
}

// ------------------------------------------------------------------ canal

/// Resuelve el canal «Mi Música» (caché → buscar → crear → re-buscar).
fn ensure_channel(handle: &Handle, client: &Client) -> Result<PeerRef, CloudError> {
    if let Some(peer) = lock_slot().channel {
        return Ok(peer);
    }
    // 1) Buscarlo entre los diálogos.
    if let Some(peer) = run(handle, 60, async {
        let mut dialogs = client.iter_dialogs();
        let mut found: Option<PeerRef> = None;
        while let Some(dialog) = dialogs.next().await.map_err(rpc_err)? {
            let peer = dialog.peer();
            let name = peer.name().map(|s| s.to_string()).unwrap_or_default();
            if name == CHANNEL_TITLE {
                if let Some(r) = peer.to_ref().await.map_err(rpc_err)? {
                    if r.id.kind() == PeerKind::Channel {
                        found = Some(r);
                        break;
                    }
                } else if peer.id().kind() == PeerKind::Channel {
                    // Sin referencia completa igual sirve: es nuestro.
                    found = Some(peer.id().to_ambient_ref());
                    break;
                }
            }
        }
        Ok(found)
    })? {
        lock_slot().channel = Some(peer);
        cache_channel_id(peer);
        return Ok(peer);
    }
    // 2) No existe: crearlo (canal privado solo nuestro).
    run(handle, 60, async {
        use grammers_tl_types as tl;
        let _: tl::enums::Updates = client
            .invoke(&tl::functions::channels::CreateChannel {
                broadcast: true,
                megagroup: false,
                title: CHANNEL_TITLE.to_string(),
                about: "Mi nube musical (Titan)".to_string(),
                geo_point: None,
                address: None,
                for_import: false,
                forum: false,
                ttl_period: None,
            })
            .await
            .map_err(|e| {
                CloudError::Channel(format!(
                    "no se pudo crear «{CHANNEL_TITLE}» ({e}); créalo a mano en Telegram y reintenta"
                ))
            })?;
        Ok(())
    })?;
    // 3) Re-buscarlo ya creado.
    let peer = run(handle, 60, async {
        let mut dialogs = client.iter_dialogs();
        let mut found: Option<PeerRef> = None;
        while let Some(dialog) = dialogs.next().await.map_err(rpc_err)? {
            let peer = dialog.peer();
            let name = peer.name().map(|s| s.to_string()).unwrap_or_default();
            if name == CHANNEL_TITLE {
                if let Some(r) = peer.to_ref().await.map_err(rpc_err)? {
                    found = Some(r);
                } else {
                    found = Some(peer.id().to_ambient_ref());
                }
                break;
            }
        }
        found.ok_or_else(|| {
            CloudError::Channel(format!(
                "se creó «{CHANNEL_TITLE}» pero no aparece; reintenta en unos segundos"
            ))
        })
    })?;
    lock_slot().channel = Some(peer);
    cache_channel_id(peer);
    Ok(peer)
}

fn cache_channel_id(peer: PeerRef) {
    let mut cfg = load_config();
    cfg.channel_id = Some(peer.id.bare_id());
    save_config(&cfg).ok();
}

// --------------------------------------------------------------- biblioteca

/// Una pista nube lista para Titan.
#[derive(Debug, Clone)]
pub struct CloudTrack {
    pub id: i32,
    pub title: String,
    pub artist: String,
    pub duration_secs: f64,
    pub size: u64,
    pub file: String,
    pub mime: String,
}

/// Lee la biblioteca del canal (y refresca la caché de `cloud:<id>`).
pub fn library() -> Result<Vec<CloudTrack>, CloudError> {
    let (handle, client) = ensure_client()?;
    require_auth(&handle, &client)?;
    let channel = ensure_channel(&handle, &client)?;
    let found = run(&handle, 120, async {
        let mut out: Vec<(CloudTrack, Media)> = Vec::new();
        let mut messages = client.iter_messages(channel);
        while let Some(msg) = messages.next().await.map_err(rpc_err)? {
            if let Some(media) = msg.media() {
                if let Some(track) = track_from_media(msg.id(), &msg.text(), &media) {
                    out.push((track, media));
                }
            }
        }
        Ok(out)
    })?;
    let mut guard = lock_slot();
    guard.refs.clear();
    let mut tracks = Vec::with_capacity(found.len());
    for (track, media) in found {
        guard.refs.insert(
            track.id,
            CloudRef {
                msg_id: track.id,
                title: track.title.clone(),
                artist: track.artist.clone(),
                duration_secs: track.duration_secs,
                size: track.size,
                file: track.file.clone(),
                mime: track.mime.clone(),
                media,
            },
        );
        tracks.push(track);
    }
    Ok(tracks)
}

fn require_auth(handle: &Handle, client: &Client) -> Result<(), CloudError> {
    let ok = run(handle, 30, async {
        Ok(client.is_authorized().await.map_err(rpc_err)?)
    })?;
    if ok {
        Ok(())
    } else {
        Err(CloudError::Auth(
            "sin login: cloud_login_phone → cloud_login_code".to_string(),
        ))
    }
}

/// Extrae título/artista/duración de un documento de audio.
fn track_from_media(msg_id: i32, caption: &str, media: &Media) -> Option<CloudTrack> {
    use grammers_tl_types as tl;
    let doc = match media {
        Media::Document(doc) => doc,
        _ => return None,
    };
    let mime = doc.mime_type().unwrap_or("application/octet-stream");
    if !is_audio_mime(mime, caption) {
        return None;
    }
    let size = Downloadable::size(doc).unwrap_or(0) as u64;
    let mut title: Option<String> = None;
    let mut artist: Option<String> = None;
    let mut duration = 0.0f64;
    let mut file = String::new();
    if let Some(tl::enums::Document::Document(raw)) = doc.raw.document.as_ref() {
        for attr in raw.attributes.iter() {
            match attr {
                tl::enums::DocumentAttribute::Audio(audio) => {
                    duration = audio.duration.max(0) as f64;
                    if title.is_none() {
                        title = audio.title.clone();
                    }
                    if artist.is_none() {
                        artist = audio.performer.clone();
                    }
                }
                tl::enums::DocumentAttribute::Filename(named) => {
                    if file.is_empty() {
                        file = named.file_name.clone();
                    }
                }
                _ => {}
            }
        }
    }
    if file.is_empty() {
        file = format!("pista-{msg_id}");
    }
    let title = title
        .filter(|t| !t.trim().is_empty())
        .or_else(|| stem_of(&file))
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| {
            if caption.trim().is_empty() {
                format!("Pista {msg_id}")
            } else {
                caption.trim().to_string()
            }
        });
    Some(CloudTrack {
        id: msg_id,
        title,
        artist: artist.unwrap_or_else(|| "Desconocido".to_string()),
        duration_secs: duration,
        size,
        file,
        mime: mime.to_string(),
    })
}

fn is_audio_mime(mime: &str, caption: &str) -> bool {
    if mime.starts_with("audio/") {
        return true;
    }
    // Telegram a veces sube música como video/octet-stream; el nombre manda.
    const EXTS: [&str; 7] = [".mp3", ".flac", ".ogg", ".oga", ".opus", ".wav", ".m4a"];
    let lower = caption.to_ascii_lowercase();
    EXTS.iter().any(|e| lower.ends_with(e))
}

fn stem_of(file: &str) -> Option<String> {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
    if stem.is_empty() {
        None
    } else {
        Some(stem.to_string())
    }
}

// ------------------------------------------------------ subir / bajar

#[cfg(feature = "audio_mod")]
fn tag_meta(path: &str) -> (Option<String>, Option<String>) {
    match crate::audio_tags::read_track(path) {
        Ok(info) => (info.title, info.artist),
        Err(_) => (None, None),
    }
}

#[cfg(not(feature = "audio_mod"))]
fn tag_meta(_path: &str) -> (Option<String>, Option<String>) {
    (None, None)
}

/// Sube un archivo de audio al canal. Devuelve `cloud_id <n>`.
pub fn upload(path: &str) -> Result<String, CloudError> {
    if !std::path::Path::new(path).is_file() {
        return Err(CloudError::Io(format!("no existe: {path}")));
    }
    // Duración real (para que Telegram la muestre como audio).
    let probed = crate::audio_decode::probe(path).map_err(|e| CloudError::Decode(e.to_string()))?;
    let (title, artist) = tag_meta(path);
    let file_name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio");
    let (handle, client) = ensure_client()?;
    require_auth(&handle, &client)?;
    let channel = ensure_channel(&handle, &client)?;
    let uploaded = run(&handle, 1800, async {
        client.upload_file(path).await.map_err(|e| {
            CloudError::Net(format!("falló la subida de {file_name} ({e})"))
        })
    })?;
    let caption = match (&title, &artist) {
        (Some(t), Some(a)) => format!("{a} — {t}"),
        (Some(t), None) => t.clone(),
        _ => file_name.to_string(),
    };
    let msg = run(&handle, 120, async {
        let message = InputMessage::new().text(caption).document(uploaded).attribute(
            Attribute::Audio {
                duration: Duration::from_secs(probed.duration_secs.max(0.0) as u64),
                title,
                performer: artist,
            },
        );
        client
            .send_message(channel, message)
            .await
            .map_err(|e| CloudError::Net(format!("no se publicó en el canal ({e})")))
    })?;
    // La caché quedó vieja (hay una pista nueva).
    lock_slot().refs.clear();
    Ok(format!("cloud_id {}", msg.id()))
}

/// Descarga EXPLÍCITA de una pista nube a `dest` (lo único que toca disco).
pub fn download(id: i64, dest: &str) -> Result<String, CloudError> {
    let id: i32 = id
        .try_into()
        .map_err(|_| CloudError::Track("id fuera de rango".to_string()))?;
    if dest.trim().is_empty() {
        return Err(CloudError::Io("destino vacío".to_string()));
    }
    let media = lock_slot()
        .refs
        .get(&id)
        .map(|r| r.media.clone())
        .ok_or_else(|| {
            CloudError::Track(format!("cloud:{id} no está en caché; ejecuta cloud_library primero"))
        })?;
    let (handle, client) = ensure_client()?;
    require_auth(&handle, &client)?;
    run(&handle, 1800, async {
        client
            .download_media(&media, dest)
            .await
            .map_err(|e| CloudError::Net(format!("falló la descarga ({e})")))
    })?;
    Ok(format!("guardada en {dest}"))
}

/// Borra una pista del canal (en Telegram, no en tu teléfono).
pub fn delete(id: i64) -> Result<String, CloudError> {
    let id: i32 = id
        .try_into()
        .map_err(|_| CloudError::Track("id fuera de rango".to_string()))?;
    let (handle, client) = ensure_client()?;
    require_auth(&handle, &client)?;
    run(&handle, 60, async {
        use grammers_tl_types as tl;
        let _: tl::enums::messages::AffectedMessages = client
            .invoke(&tl::functions::messages::DeleteMessages {
                revoke: true,
                id: vec![id],
            })
            .await
            .map_err(|e| CloudError::Net(format!("no se pudo borrar ({e})")))?;
        Ok(())
    })?;
    lock_slot().refs.remove(&id);
    Ok(format!("cloud:{id} borrada del canal"))
}

// -------------------------------------------------------------- streaming

/// Fuente de pedazos por offset. La real habla con Telegram; el mock
/// de los tests sirve bytes de memoria (misma forma, cero red).
pub trait ChunkSource: Send + Sync {
    fn fetch(&self, offset: u64, len: usize) -> Result<Vec<u8>, CloudError>;
    fn size(&self) -> u64;
}

/// Pedazos vía `upload.getFile` (descarga parcial, con seek).
pub struct GrammersChunks {
    handle: Handle,
    client: Client,
    media: Media,
    size: u64,
}

impl ChunkSource for GrammersChunks {
    fn size(&self) -> u64 {
        self.size
    }

    fn fetch(&self, offset: u64, len: usize) -> Result<Vec<u8>, CloudError> {
        if offset >= self.size || len == 0 {
            return Ok(Vec::new());
        }
        // Telegram pide múltiplos de 4 KiB (máx 512 KiB): se alinea
        // hacia abajo y se recorta el sobrante del inicio.
        let limit = (len.min(FETCH_CHUNK) + 4095) / 4096 * 4096;
        let limit = limit.clamp(4096, FETCH_CHUNK) as i32;
        let aligned = offset - (offset % limit as u64);
        let skip = (aligned / limit as u64) as i32;
        let trim = (offset - aligned) as usize;
        let chunk = run(&self.handle, 90, async {
            let mut dl = self
                .client
                .iter_download(&self.media)
                .chunk_size(limit)
                .skip_chunks(skip);
            let mut buf: Vec<u8> = Vec::new();
            while buf.len() < trim + len.min(FETCH_CHUNK) {
                match dl.next().await.map_err(rpc_err)? {
                    Some(part) => buf.extend_from_slice(&part),
                    None => break,
                }
            }
            Ok(buf)
        })?;
        if trim >= chunk.len() {
            return Ok(Vec::new());
        }
        let end = (trim + len).min(chunk.len());
        Ok(chunk[trim..end].to_vec())
    }
}

/// Lector seekable sobre pedazos: esto es lo que mastica symphonia.
/// Nunca guarda más de ~256 KiB + el pedazo en vuelo.
pub struct CloudMediaSource {
    src: Box<dyn ChunkSource>,
    size: u64,
    pos: u64,
    buf: VecDeque<u8>,
}

impl CloudMediaSource {
    pub fn new(src: Box<dyn ChunkSource>, size: u64) -> Self {
        CloudMediaSource {
            src,
            size,
            pos: 0,
            buf: VecDeque::new(),
        }
    }
}

impl Read for CloudMediaSource {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if out.is_empty() || self.pos >= self.size {
            return Ok(0);
        }
        while self.buf.is_empty() && self.pos < self.size {
            let want = (self.size - self.pos).min(READ_AHEAD as u64) as usize;
            let part = self
                .src
                .fetch(self.pos, want)
                .map_err(std::io::Error::other)?;
            if part.is_empty() {
                break;
            }
            self.pos += part.len() as u64;
            self.buf.extend(part);
        }
        let n = out.len().min(self.buf.len());
        for slot in out.iter_mut().take(n) {
            *slot = self.buf.pop_front().unwrap_or(0);
        }
        Ok(n)
    }
}

impl Seek for CloudMediaSource {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        let base: i64 = match from {
            // Ojo: `pos` ya incluye lo que duerme en `buf`.
            SeekFrom::Start(off) => off as i64 - self.pos as i64,
            SeekFrom::End(off) => self.size as i64 + off - self.pos as i64,
            SeekFrom::Current(off) => off - self.buf.len() as i64,
        };
        let logical = self.pos as i64 - self.buf.len() as i64 + base;
        let logical = logical.clamp(0, self.size as i64) as u64;
        self.pos = logical;
        self.buf.clear();
        Ok(logical)
    }
}

impl MediaSource for CloudMediaSource {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.size)
    }
}

// ------------------------------------------------------- puente al motor

fn describe(id: i32) -> Option<(String, f64)> {
    lock_slot().refs.get(&id).map(|r| {
        (
            format!("☁ {} — {}", r.artist, r.title),
            r.duration_secs,
        )
    })
}

fn open_stream(id: i32) -> Result<Box<dyn MediaSource>, String> {
    let (handle, client, media, size) = {
        let guard = lock_slot();
        let r = guard.refs.get(&id).ok_or_else(|| {
            format!("cloud:{id} no está en caché; ejecuta cloud_library primero")
        })?;
        let client = guard
            .client
            .clone()
            .ok_or_else(|| "sin conexión (cloud_login_phone primero)".to_string())?;
        let handle = guard
            .rt
            .as_ref()
            .map(|rt| rt.handle().clone())
            .ok_or_else(|| "sin runtime".to_string())?;
        (handle, client, r.media.clone(), r.size)
    };
    let src: Box<dyn ChunkSource> = Box::new(GrammersChunks {
        handle,
        client,
        media,
        size,
    });
    Ok(Box::new(CloudMediaSource::new(src, size)))
}

/// Conecta este módulo con el motor (una vez; idempotente).
pub fn ensure_hooks() {
    crate::audio_decode::register_cloud_hooks(
        crate::audio_decode::CloudHooks {
            open: open_stream,
            describe,
        },
    );
}

/// Mete una pista nube a la cola (`cloud:<id>`).
pub fn queue_add_cloud(id: i64) -> Result<String, CloudError> {
    let id: i32 = id
        .try_into()
        .map_err(|_| CloudError::Track("id fuera de rango".to_string()))?;
    ensure_client()?;
    if !lock_slot().refs.contains_key(&id) {
        return Err(CloudError::Track(format!(
            "cloud:{id} no está en caché; ejecuta cloud_library primero"
        )));
    }
    crate::audio_engine::queue_add(&format!("{CLOUD_PREFIX}{id}"))
        .map_err(|e| CloudError::Engine(e.to_string()))
}

/// Toca una pista nube ya mismo.
pub fn play_cloud(id: i64) -> Result<String, CloudError> {
    let id: i32 = id
        .try_into()
        .map_err(|_| CloudError::Track("id fuera de rango".to_string()))?;
    ensure_client()?;
    if !lock_slot().refs.contains_key(&id) {
        return Err(CloudError::Track(format!(
            "cloud:{id} no está en caché; ejecuta cloud_library primero"
        )));
    }
    crate::audio_engine::play(&format!("{CLOUD_PREFIX}{id}"))
        .map_err(|e| CloudError::Engine(e.to_string()))
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Fuente falsa: bytes de memoria contando los pedidos.
    struct MockChunks {
        data: Vec<u8>,
        fetches: AtomicUsize,
    }

    impl ChunkSource for MockChunks {
        fn size(&self) -> u64 {
            self.data.len() as u64
        }

        fn fetch(&self, offset: u64, len: usize) -> Result<Vec<u8>, CloudError> {
            self.fetches.fetch_add(1, Ordering::SeqCst);
            let start = offset.min(self.data.len() as u64) as usize;
            let end = start.saturating_add(len).min(self.data.len());
            Ok(self.data[start..end].to_vec())
        }
    }

    fn mock_source(data: Vec<u8>) -> (CloudMediaSource, Arc<MockChunks>) {
        let mock = Arc::new(MockChunks {
            data,
            fetches: AtomicUsize::new(0),
        });
        struct Wrap(Arc<MockChunks>);
        impl ChunkSource for Wrap {
            fn size(&self) -> u64 {
                self.0.size()
            }
            fn fetch(&self, offset: u64, len: usize) -> Result<Vec<u8>, CloudError> {
                self.0.fetch(offset, len)
            }
        }
        let size = mock.size();
        (CloudMediaSource::new(Box::new(Wrap(mock.clone())), size), mock)
    }

    #[test]
    fn stream_reads_sequentially() {
        let data: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        let (mut src, _mock) = mock_source(data.clone());
        let mut out = vec![0u8; data.len()];
        let mut got = 0;
        while got < out.len() {
            let n = src.read(&mut out[got..]).unwrap();
            assert!(n > 0, "se cortó en {got}");
            got += n;
        }
        assert_eq!(out, data);
        // EOF honesto.
        assert_eq!(src.read(&mut out[..10]).unwrap(), 0);
    }

    #[test]
    fn stream_seeks_both_ways() {
        let data: Vec<u8> = (0..50_000u32).map(|i| (i * 7 % 251) as u8).collect();
        let (mut src, _mock) = mock_source(data.clone());
        // Adelante desalineado.
        assert_eq!(src.seek(SeekFrom::Start(12_345)).unwrap(), 12_345);
        let mut one = [0u8; 1];
        src.read_exact(&mut one).unwrap();
        assert_eq!(one[0], data[12_345]);
        // Atrás.
        assert_eq!(src.seek(SeekFrom::Start(7)).unwrap(), 7);
        src.read_exact(&mut one).unwrap();
        assert_eq!(one[0], data[7]);
        // Relativo y desde el final.
        assert_eq!(src.seek(SeekFrom::Current(100)).unwrap(), 108);
        let end = src.seek(SeekFrom::End(-1)).unwrap();
        assert_eq!(end, 49_999);
        src.read_exact(&mut one).unwrap();
        assert_eq!(one[0], data[49_999]);
        // Más allá del final = pinzado.
        assert_eq!(src.seek(SeekFrom::Start(99_999_999)).unwrap(), 50_000);
    }

    fn pcm_wav_bytes(secs: f64) -> Vec<u8> {
        // WAV mono 16-bit 22050 Hz, seno 440 Hz (igual que el engine).
        let rate = 22_050u32;
        let n = (secs * rate as f64) as usize;
        let mut bytes = Vec::with_capacity(44 + n * 2);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&((36 + n * 2) as u32).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(n * 2u32 as usize).to_le_bytes());
        for i in 0..n {
            let t = i as f64 / rate as f64;
            let s = (2.0 * std::f64::consts::PI * 440.0 * t).sin();
            bytes.extend_from_slice(&((s * 20000.0) as i16).to_le_bytes());
        }
        bytes
    }

    #[test]
    fn streamed_wav_decodes_end_to_end() {
        // La prueba reina: symphonia decodificando un WAV que llega
        // por pedazos, como si viniera de Telegram (con seek real).
        let data = pcm_wav_bytes(1.0);
        let (src, mock) = mock_source(data);
        let mut decoder =
            crate::audio_decode::FileDecoder::open_source(Box::new(src), "nube", Some("wav"))
                .expect("probe sobre stream");
        assert_eq!(decoder.sample_rate(), 22_050);
        assert_eq!(decoder.channels(), 1);
        let mut pcm: Vec<f32> = Vec::new();
        while !decoder.is_finished() {
            decoder.fill(&mut pcm, 4096).expect("fill");
        }
        assert!(pcm.len() > 20_000, "samples={}", pcm.len());
        assert!(mock.fetches.load(Ordering::SeqCst) > 0);
        // Y el seek-a-tiempo (lo que usa engine_seek) también va.
        let data = pcm_wav_bytes(1.0);
        let (src, _) = mock_source(data);
        let mut decoder =
            crate::audio_decode::FileDecoder::open_source(Box::new(src), "nube", Some("wav"))
                .expect("re-probe");
        decoder.seek(0.5).expect("seek");
        let mut pcm2: Vec<f32> = Vec::new();
        decoder.fill(&mut pcm2, 4096).expect("fill");
        assert!(!pcm2.is_empty());
    }

    #[test]
    fn session_file_roundtrip() {
        let dir = std::env::temp_dir().join(format!("titan-tg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.json");
        // Sesión con de todo, vía el trait real.
        let sess = JsonFileSession::open(path.clone());
        let rt = Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            use grammers_session::types::PeerId;
            sess.set_home_dc_id(4).await.unwrap();
            let _ = sess.home_dc_id().unwrap();
            // Un peer inventado (canal 123) vía import_to.
            let mut data = SessionData::default();
            data.home_dc = 4;
            data.import_to(&sess).await.unwrap();
            let back = sess.peer(PeerId::channel(123).unwrap()).await.unwrap();
            assert!(back.is_none());
        });
        drop(sess);
        // El archivo existe y recarga lo mismo.
        assert!(path.is_file());
        let again = JsonFileSession::open(path.clone());
        assert_eq!(again.home_dc_id().unwrap(), 4);
        rt.block_on(async {
            let st = again.updates_state().await.unwrap();
            assert_eq!(st.pts, 0);
        });
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn track_metadata_from_raw_document() {
        use grammers_tl_types as tl;
        // Documento fabricado a mano: MP3 con etiqueta de audio.
        let raw_doc = tl::types::Document {
            id: 777,
            access_hash: 888,
            file_reference: vec![1, 2, 3],
            date: 1_700_000_000,
            mime_type: "audio/mpeg".to_string(),
            size: 4_000_000,
            thumbs: None,
            video_thumbs: None,
            dc_id: 4,
            attributes: vec![
                tl::enums::DocumentAttribute::Audio(tl::types::DocumentAttributeAudio {
                    voice: false,
                    duration: 211,
                    title: Some("Mi Canción".to_string()),
                    performer: Some("La Banda".to_string()),
                    waveform: None,
                }),
                tl::enums::DocumentAttribute::Filename(tl::types::DocumentAttributeFilename {
                    file_name: "la_banda-mi_cancion.mp3".to_string(),
                }),
            ],
        };
        let media = Media::Document(grammers_client::media::Document::from_raw_media(
            tl::types::MessageMediaDocument {
                flags: 0,
                document: Some(tl::enums::Document::Document(raw_doc)),
                ttl_seconds: None,
            },
        ));
        let track = track_from_media(42, "", &media).expect("audio válido");
        assert_eq!(track.id, 42);
        assert_eq!(track.title, "Mi Canción");
        assert_eq!(track.artist, "La Banda");
        assert_eq!(track.duration_secs, 211.0);
        assert_eq!(track.file, "la_banda-mi_cancion.mp3");
        assert_eq!(track.mime, "audio/mpeg");
        assert_eq!(track.size, 4_000_000);
        // Lo que no es audio se ignora.
        let photo_media = Media::Photo(grammers_client::media::Photo::from_raw(
            tl::enums::Photo::Empty(tl::types::PhotoEmpty { id: 1 }),
        ));
        assert!(track_from_media(43, "foto", &photo_media).is_none());
    }

    #[test]
    fn errors_speak_spanish() {
        assert!(CloudError::Setup("x".into()).to_string().contains("falta"));
        assert!(CloudError::Auth("x".into()).to_string().contains("login"));
        let id_display = format!("{CLOUD_PREFIX}9");
        assert_eq!(id_display, "cloud:9");
    }
}
