mod text;

use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use tokio::sync::Mutex;
use zbus::connection::Builder;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, interface};

use crate::engine::{Action, KeyEvent, LisleEngine};
use text::ibus_text;

pub const COMPONENT_NAME: &str = "org.freedesktop.IBus.Lisle";
pub const ENGINE_NAME: &str = "lisle";
pub const FACTORY_PATH: &str = "/org/freedesktop/IBus/Factory";

const PREEDIT_CLEAR: u32 = 0;
const PREEDIT_COMMIT: u32 = 1;

type BoxError = Box<dyn Error + Send + Sync>;
type Core = Arc<Mutex<LisleEngine>>;

pub async fn run() -> Result<(), BoxError> {
    let connection = connect().await?;
    register(&connection).await?;
    connection.request_name(COMPONENT_NAME).await?;

    tokio::select! {
        () = connection.closed() => {}
        result = tokio::signal::ctrl_c() => {
            result?;
            connection.close().await?;
        }
    }
    Ok(())
}

async fn register(connection: &Connection) -> zbus::Result<()> {
    connection
        .object_server()
        .at(FACTORY_PATH, Factory::new(connection.clone()))
        .await?;
    connection
        .object_server()
        .at(
            FACTORY_PATH,
            Service::new(connection.clone(), FACTORY_PATH.try_into()?, None),
        )
        .await?;
    Ok(())
}

async fn connect() -> zbus::Result<Connection> {
    let address = explicit_address();
    match address {
        Some(address) => Builder::address(address.as_str())?.build().await,
        None => Builder::ibus()?.build().await,
    }
}

fn explicit_address() -> Option<String> {
    if let Ok(address) = std::env::var("IBUS_ADDRESS")
        && !address.is_empty()
    {
        return Some(address);
    }

    let path = std::env::var_os("IBUS_ADDRESS_FILE")
        .map(PathBuf::from)
        .or_else(default_address_file)?;
    let contents = fs::read_to_string(path).ok()?;
    parse_address_file(&contents)
}

fn default_address_file() -> Option<PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    let display = std::env::var_os("WAYLAND_DISPLAY")?;
    if display.is_empty() {
        return None;
    }
    let machine_id = ["/var/lib/dbus/machine-id", "/etc/machine-id"]
        .into_iter()
        .find_map(|path| fs::read_to_string(path).ok())?;
    let machine_id = machine_id.trim();
    if machine_id.is_empty() {
        return None;
    }

    Some(
        config_home
            .join("ibus/bus")
            .join(format!("{machine_id}-unix-{}", display.to_string_lossy())),
    )
}

fn parse_address_file(contents: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        line.trim()
            .strip_prefix("IBUS_ADDRESS=")
            .map(|address| address.trim_matches(['\'', '"']).to_owned())
            .filter(|address| !address.is_empty())
    })
}

struct Factory {
    connection: Connection,
    next_engine: AtomicU32,
}

impl Factory {
    fn new(connection: Connection) -> Self {
        Self {
            connection,
            next_engine: AtomicU32::new(1),
        }
    }
}

#[interface(name = "org.freedesktop.IBus.Factory", spawn = false)]
impl Factory {
    async fn create_engine(&self, name: &str) -> zbus::fdo::Result<OwnedObjectPath> {
        if name != ENGINE_NAME {
            return Err(zbus::fdo::Error::Failed(format!(
                "unknown Lisle engine: {name}"
            )));
        }

        let number = self.next_engine.fetch_add(1, Ordering::Relaxed);
        let path = OwnedObjectPath::try_from(format!("/org/freedesktop/IBus/Engine/{number}"))
            .map_err(fdo_error)?;
        let emitter = SignalEmitter::new(&self.connection, path.clone())
            .map_err(fdo_error)?
            .into_owned();
        let core = Arc::new(Mutex::new(LisleEngine::default()));
        self.connection
            .object_server()
            .at(path.clone(), IbusEngine::new(core.clone(), emitter))
            .await
            .map_err(fdo_error)?;
        self.connection
            .object_server()
            .at(
                path.clone(),
                Service::new(self.connection.clone(), path.clone(), Some(core)),
            )
            .await
            .map_err(fdo_error)?;
        Ok(path)
    }
}

struct Service {
    connection: Connection,
    path: OwnedObjectPath,
    core: Option<Core>,
}

impl Service {
    fn new(connection: Connection, path: OwnedObjectPath, core: Option<Core>) -> Self {
        Self {
            connection,
            path,
            core,
        }
    }
}

#[interface(name = "org.freedesktop.IBus.Service", spawn = false)]
impl Service {
    async fn destroy(&self) {
        if let Some(core) = &self.core {
            core.lock().await.end_context();
        }

        let connection = self.connection.clone();
        let path = self.path.clone();
        let factory = self.core.is_none();
        tokio::spawn(async move {
            tokio::task::yield_now().await;
            let server = connection.object_server();
            if factory {
                let _ = server.remove::<Factory, _>(path.clone()).await;
            } else {
                let _ = server.remove::<IbusEngine, _>(path.clone()).await;
            }
            let _ = server.remove::<Service, _>(path).await;
            if factory {
                let _ = connection.close().await;
            }
        });
    }
}

struct IbusEngine {
    core: Core,
    emitter: SignalEmitter<'static>,
    content_type: (u32, u32),
}

impl IbusEngine {
    fn new(core: Core, emitter: SignalEmitter<'static>) -> Self {
        Self {
            core,
            emitter,
            content_type: (0, 0),
        }
    }

    async fn update_preedit(&self, text: &str) -> zbus::Result<()> {
        let value = ibus_text(text);
        let mode = if text.is_empty() {
            PREEDIT_CLEAR
        } else {
            PREEDIT_COMMIT
        };
        Self::update_preedit_text(
            &self.emitter,
            &value,
            text.chars().count() as u32,
            !text.is_empty(),
            mode,
        )
        .await
    }

    async fn emit_actions(&self, actions: Vec<Action>) -> zbus::Result<()> {
        for action in actions {
            match action {
                Action::Commit(text) => {
                    let value = ibus_text(text);
                    Self::commit_text(&self.emitter, &value).await?;
                }
                Action::Preedit(text) => self.update_preedit(&text).await?,
                Action::Forward {
                    keyval,
                    keycode,
                    state,
                } => {
                    Self::forward_key_event(&self.emitter, keyval, keycode, state).await?;
                }
            }
        }
        Ok(())
    }

    async fn start_context(&self) -> zbus::fdo::Result<()> {
        let mut core = self.core.lock().await;
        core.focus_in();
        self.update_preedit("").await.map_err(fdo_error)
    }

    async fn reset_context(&self) -> zbus::fdo::Result<()> {
        let mut core = self.core.lock().await;
        core.reset();
        self.update_preedit("").await.map_err(fdo_error)
    }

    async fn end_context(&self) {
        self.core.lock().await.end_context();
    }
}

#[interface(name = "org.freedesktop.IBus.Engine", spawn = false)]
impl IbusEngine {
    async fn process_key_event(
        &self,
        keyval: u32,
        keycode: u32,
        state: u32,
    ) -> zbus::fdo::Result<bool> {
        let mut core = self.core.lock().await;
        let (handled, actions) = core.process(KeyEvent::new(keyval, keycode, state));
        if let Err(error) = self.emit_actions(actions).await {
            core.reset();
            return Err(fdo_error(error));
        }
        Ok(handled)
    }

    async fn focus_in(&self) -> zbus::fdo::Result<()> {
        self.start_context().await
    }

    async fn focus_in_id(&self, _object_path: &str, _client: &str) -> zbus::fdo::Result<()> {
        self.start_context().await
    }

    async fn focus_out(&self) {
        self.end_context().await;
    }

    async fn focus_out_id(&self, _object_path: &str) {
        self.end_context().await;
    }

    async fn reset(&self) -> zbus::fdo::Result<()> {
        self.reset_context().await
    }

    async fn enable(&self) -> zbus::fdo::Result<()> {
        self.start_context().await
    }

    async fn disable(&self) {
        self.end_context().await;
    }

    fn set_cursor_location(&self, _x: i32, _y: i32, _w: i32, _h: i32) {}

    fn process_hand_writing_event(&self, _coordinates: Vec<f64>) {}

    fn cancel_hand_writing(&self, _n_strokes: u32) {}

    fn set_capabilities(&self, _caps: u32) {}

    fn property_activate(&self, _name: &str, _state: u32) {}

    fn property_show(&self, _name: &str) {}

    fn property_hide(&self, _name: &str) {}

    fn candidate_clicked(&self, _index: u32, _button: u32, _state: u32) {}

    fn page_up(&self) {}

    fn page_down(&self) {}

    fn cursor_up(&self) {}

    fn cursor_down(&self) {}

    fn set_surrounding_text(&self, _text: OwnedValue, _cursor: u32, _anchor: u32) {}

    fn panel_extension_received(&self, _event: OwnedValue) {}

    fn panel_extension_register_keys(&self, _data: OwnedValue) {}

    #[zbus(property)]
    fn set_content_type(&mut self, value: (u32, u32)) {
        self.content_type = value;
    }

    #[zbus(property)]
    fn focus_id(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn active_surrounding_text(&self) -> bool {
        false
    }

    #[zbus(signal)]
    async fn commit_text(emitter: &SignalEmitter<'_>, text: &Value<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn update_preedit_text(
        emitter: &SignalEmitter<'_>,
        text: &Value<'_>,
        cursor: u32,
        visible: bool,
        mode: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn update_auxiliary_text(
        emitter: &SignalEmitter<'_>,
        text: &Value<'_>,
        visible: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn update_lookup_table(
        emitter: &SignalEmitter<'_>,
        table: &Value<'_>,
        visible: bool,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn register_properties(
        emitter: &SignalEmitter<'_>,
        properties: &Value<'_>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn update_property(emitter: &SignalEmitter<'_>, property: &Value<'_>)
    -> zbus::Result<()>;

    #[zbus(signal)]
    async fn forward_key_event(
        emitter: &SignalEmitter<'_>,
        keyval: u32,
        keycode: u32,
        state: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn panel_extension(emitter: &SignalEmitter<'_>, data: &Value<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn send_message(emitter: &SignalEmitter<'_>, message: &Value<'_>) -> zbus::Result<()>;
}

fn fdo_error(error: impl std::fmt::Display) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use tokio::net::UnixStream;
    use zbus::Guid;
    use zbus::fdo::IntrospectableProxy;

    #[test]
    fn address_file_parser_accepts_ibus_assignment() {
        assert_eq!(
            parse_address_file("IBUS_ADDRESS='unix:path=/tmp/ibus-test,guid=1'\n").as_deref(),
            Some("unix:path=/tmp/ibus-test,guid=1")
        );
        assert_eq!(parse_address_file("unrelated=value\n"), None);
    }

    async fn peer_connections() -> (Connection, Connection) {
        let (server_stream, client_stream) = UnixStream::pair().expect("Unix socket pair");
        let guid = Guid::generate();
        let server = Builder::unix_stream(server_stream)
            .server(guid)
            .expect("server GUID")
            .p2p()
            .build();
        let client = Builder::unix_stream(client_stream).p2p().build();
        futures_util::try_join!(server, client).expect("peer D-Bus connections")
    }

    #[tokio::test]
    async fn factory_engine_and_service_match_ibus_wire_contract() {
        let (server, client) = peer_connections().await;
        register(&server).await.expect("register IBus objects");

        let factory = zbus::Proxy::new(
            &client,
            COMPONENT_NAME,
            FACTORY_PATH,
            "org.freedesktop.IBus.Factory",
        )
        .await
        .expect("factory proxy");
        let path: OwnedObjectPath = factory
            .call("CreateEngine", &(ENGINE_NAME,))
            .await
            .expect("create engine");
        assert_eq!(path.as_str(), "/org/freedesktop/IBus/Engine/1");

        let introspection = IntrospectableProxy::builder(&client)
            .destination(COMPONENT_NAME)
            .expect("destination")
            .path(path.clone())
            .expect("engine path")
            .build()
            .await
            .expect("introspection proxy")
            .introspect()
            .await
            .expect("engine introspection");
        for expected in [
            "org.freedesktop.IBus.Engine",
            "org.freedesktop.IBus.Service",
            "<method name=\"ProcessKeyEvent\">",
            "<method name=\"FocusInId\">",
            "<method name=\"PanelExtensionRegisterKeys\">",
            "<property name=\"ContentType\" type=\"(uu)\" access=\"write\">",
            "<property name=\"FocusId\" type=\"b\" access=\"read\"",
            "<property name=\"ActiveSurroundingText\" type=\"b\" access=\"read\"",
            "<signal name=\"UpdatePreeditText\">",
        ] {
            assert!(
                introspection.contains(expected),
                "missing {expected} in:\n{introspection}"
            );
        }

        let engine = zbus::Proxy::new(
            &client,
            COMPONENT_NAME,
            path.clone(),
            "org.freedesktop.IBus.Engine",
        )
        .await
        .expect("engine proxy");
        let mut commits = engine
            .receive_signal("CommitText")
            .await
            .expect("commit signal stream");
        let handled: bool = engine
            .call("ProcessKeyEvent", &(b'e' as u32, 18_u32, 0_u32))
            .await
            .expect("Roman key event");
        assert!(handled);
        let commit = tokio::time::timeout(std::time::Duration::from_secs(1), commits.next())
            .await
            .expect("commit signal timeout")
            .expect("commit signal");
        let value: OwnedValue = commit.body().deserialize().expect("IBusText signal value");
        assert_eq!(value.value_signature().to_string(), "(sa{sv}sv)");

        let mut preedits = engine
            .receive_signal("UpdatePreeditText")
            .await
            .expect("preedit signal stream");
        for (keyval, keycode, state) in [
            (
                crate::engine::keysym::SHIFT_R,
                54_u32,
                crate::engine::SHIFT_MASK,
            ),
            (
                crate::engine::keysym::SHIFT_R,
                54_u32,
                crate::engine::SHIFT_MASK | crate::engine::RELEASE_MASK,
            ),
            (b'k' as u32, 37_u32, 0_u32),
        ] {
            let handled: bool = engine
                .call("ProcessKeyEvent", &(keyval, keycode, state))
                .await
                .expect("Hangul key event");
            assert!(handled);
        }
        let preedit = tokio::time::timeout(std::time::Duration::from_secs(1), preedits.next())
            .await
            .expect("preedit signal timeout")
            .expect("preedit signal");
        let (value, cursor, visible, mode): (OwnedValue, u32, bool, u32) =
            preedit.body().deserialize().expect("preedit signal body");
        assert_eq!(value.value_signature().to_string(), "(sa{sv}sv)");
        assert_eq!((cursor, visible, mode), (1, true, PREEDIT_COMMIT));

        let _: () = engine.call("Reset", &()).await.expect("reset engine");
        let cleared = tokio::time::timeout(std::time::Duration::from_secs(1), preedits.next())
            .await
            .expect("clear signal timeout")
            .expect("clear signal");
        let (_, cursor, visible, mode): (OwnedValue, u32, bool, u32) =
            cleared.body().deserialize().expect("clear signal body");
        assert_eq!((cursor, visible, mode), (0, false, PREEDIT_CLEAR));

        let handled: bool = engine
            .call(
                "ProcessKeyEvent",
                &(b'f' as u32, 18_u32, crate::engine::CONTROL_MASK),
            )
            .await
            .expect("shortcut event");
        assert!(!handled);

        let service = zbus::Proxy::new(
            &client,
            COMPONENT_NAME,
            path.clone(),
            "org.freedesktop.IBus.Service",
        )
        .await
        .expect("service proxy");
        service
            .call_method("Destroy", &())
            .await
            .expect("destroy engine");
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        assert!(
            IntrospectableProxy::builder(&client)
                .destination(COMPONENT_NAME)
                .expect("destination")
                .path(path)
                .expect("engine path")
                .build()
                .await
                .expect("introspection proxy")
                .introspect()
                .await
                .is_err()
        );
    }
}
