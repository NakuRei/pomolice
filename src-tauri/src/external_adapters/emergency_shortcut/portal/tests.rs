use super::*;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use zbus::message::Header;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

type Options = HashMap<String, OwnedValue>;
type Shortcuts = Vec<(String, Options)>;

struct TestBus(Child);

impl Drop for TestBus {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Portal {
    response: u32,
    session: Arc<parking_lot::Mutex<Option<OwnedObjectPath>>>,
    app_id: &'static str,
    version: u32,
}

fn string_value(value: &str) -> OwnedValue {
    Value::from(value).try_to_owned().expect("string value")
}

fn shortcuts(trigger: &str) -> Shortcuts {
    vec![(
        SHORTCUT_ID.to_owned(),
        HashMap::from([
            ("description".to_owned(), string_value("Emergency release")),
            ("trigger_description".to_owned(), string_value(trigger)),
        ]),
    )]
}

fn request_path(header: &Header<'_>, token: &OwnedValue) -> OwnedObjectPath {
    let sender = header
        .sender()
        .expect("bus sender")
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let token = <&str>::try_from(token).expect("handle token");
    format!("/org/freedesktop/portal/desktop/request/{sender}/{token}")
        .try_into()
        .expect("request path")
}

#[zbus::interface(name = "org.freedesktop.host.portal.Registry")]
impl Portal {
    fn register(&self, app_id: &str, options: Options) -> zbus::fdo::Result<()> {
        drop(options);
        if app_id == self.app_id {
            Ok(())
        } else {
            Err(zbus::fdo::Error::InvalidArgs(
                "unexpected application".into(),
            ))
        }
    }
}

struct ShortcutsPortal(Portal);

#[zbus::interface(name = "org.freedesktop.portal.GlobalShortcuts")]
impl ShortcutsPortal {
    #[zbus(property)]
    fn version(&self) -> u32 {
        self.0.version
    }

    async fn create_session(
        &self,
        options: Options,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let request = request_path(&header, &options["handle_token"]);
        let sender = header
            .sender()
            .expect("sender")
            .as_str()
            .trim_start_matches(':')
            .replace('.', "_");
        let token = <&str>::try_from(&options["session_handle_token"]).expect("session token");
        let session: OwnedObjectPath =
            format!("/org/freedesktop/portal/desktop/session/{sender}/{token}")
                .try_into()
                .expect("session path");
        connection
            .object_server()
            .at(session.clone(), TestSession { closed: false })
            .await?;
        *self.0.session.lock() = Some(session.clone());
        connection
            .emit_signal(
                header.sender().cloned(),
                request.clone(),
                "org.freedesktop.portal.Request",
                "Response",
                &(
                    0_u32,
                    HashMap::from([("session_handle", string_value(session.as_str()))]),
                ),
            )
            .await?;
        Ok(request)
    }

    async fn bind_shortcuts(
        &self,
        session: OwnedObjectPath,
        requested: Shortcuts,
        parent_window: &str,
        options: Options,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        assert_eq!(Some(session), *self.0.session.lock());
        let _ = parent_window;
        assert_eq!(requested.len(), 1);
        assert_eq!(requested[0].0, SHORTCUT_ID);
        assert_eq!(
            <&str>::try_from(&requested[0].1["preferred_trigger"]).expect("trigger"),
            "CTRL+ALT+SHIFT+e"
        );
        let request = request_path(&header, &options["handle_token"]);
        let values = Value::from(shortcuts("Super + F12"))
            .try_to_owned()
            .expect("shortcut values");
        connection
            .emit_signal(
                header.sender().cloned(),
                request.clone(),
                "org.freedesktop.portal.Request",
                "Response",
                &(self.0.response, HashMap::from([("shortcuts", values)])),
            )
            .await?;
        Ok(request)
    }
}

struct TestSession {
    closed: bool,
}

#[zbus::interface(name = "org.freedesktop.portal.Session")]
impl TestSession {
    fn close(&mut self) {
        self.closed = true;
    }
}

struct Fixture {
    _bus: TestBus,
    server: zbus::Connection,
    session: Arc<parking_lot::Mutex<Option<OwnedObjectPath>>>,
    status: mpsc::UnboundedReceiver<EmergencyShortcutStatus>,
    activations: mpsc::UnboundedReceiver<()>,
    task: tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
}

impl Fixture {
    async fn new(response: u32) -> Self {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon must be installed for portal contract tests");
        let mut address = String::new();
        BufReader::new(child.stdout.take().expect("stdout"))
            .read_line(&mut address)
            .expect("bus address");
        let bus = TestBus(child);
        let session = Arc::new(parking_lot::Mutex::new(None));
        let server = zbus::connection::Builder::address(address.trim())
            .expect("address")
            .name("org.freedesktop.portal.Desktop")
            .expect("name")
            .serve_at(
                "/org/freedesktop/portal/desktop",
                Portal {
                    response,
                    session: Arc::clone(&session),
                    app_id: "io.github.NakuRei.pomolice",
                    version: 2,
                },
            )
            .expect("registry")
            .serve_at(
                "/org/freedesktop/portal/desktop",
                ShortcutsPortal(Portal {
                    response,
                    session: Arc::clone(&session),
                    app_id: "io.github.NakuRei.pomolice",
                    version: 2,
                }),
            )
            .expect("shortcuts")
            .build()
            .await
            .expect("server");
        let client = zbus::connection::Builder::address(address.trim())
            .expect("address")
            .build()
            .await
            .expect("client");
        let (status_tx, status) = mpsc::unbounded_channel();
        let (activation_tx, activations) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            run(
                client,
                "io.github.NakuRei.pomolice",
                move |status| {
                    let _ = status_tx.send(status);
                },
                move || {
                    let _ = activation_tx.send(());
                },
            )
            .await
        });
        Self {
            _bus: bus,
            server,
            session,
            status,
            activations,
            task,
        }
    }

    fn session_path(&self) -> OwnedObjectPath {
        self.session.lock().clone().expect("created session")
    }

    async fn next_status(&mut self) -> EmergencyShortcutStatus {
        tokio::time::timeout(Duration::from_secs(5), self.status.recv())
            .await
            .expect("status timeout")
            .expect("status")
    }

    async fn activate(&self, id: &str) {
        self.server
            .emit_signal(
                None::<&str>,
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.GlobalShortcuts",
                "Activated",
                &(self.session_path(), id, 1_u64, Options::new()),
            )
            .await
            .expect("activation");
    }

    async fn change_shortcut(&self, trigger: &str) {
        self.server
            .emit_signal(
                None::<&str>,
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.GlobalShortcuts",
                "ShortcutsChanged",
                &(self.session_path(), shortcuts(trigger)),
            )
            .await
            .expect("shortcut change");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn assigned_shortcut_activates_without_a_window_or_input_monitor_and_tracks_changes() {
    let mut fixture = Fixture::new(0).await;
    assert_eq!(
        fixture.next_status().await,
        EmergencyShortcutStatus::Registered {
            trigger: "Super + F12".into()
        }
    );
    fixture.activate("unrelated-action").await;
    fixture.activate(SHORTCUT_ID).await;
    tokio::time::timeout(Duration::from_secs(5), fixture.activations.recv())
        .await
        .expect("activation timeout")
        .expect("activation");
    fixture.change_shortcut("Ctrl + F10").await;
    assert_eq!(
        fixture.next_status().await,
        EmergencyShortcutStatus::Registered {
            trigger: "Ctrl + F10".into()
        }
    );
    assert!(
        fixture.activations.try_recv().is_err(),
        "unrelated actions must not release the break"
    );
    fixture.change_shortcut("").await;
    assert_eq!(
        fixture.next_status().await,
        EmergencyShortcutStatus::Unavailable
    );
}

#[tokio::test]
async fn denied_registration_never_advertises_a_working_shortcut() {
    let mut fixture = Fixture::new(1).await;
    let result = tokio::time::timeout(Duration::from_secs(5), &mut fixture.task)
        .await
        .expect("registration timeout")
        .expect("task");
    assert!(result.is_err());
    assert_eq!(
        fixture.next_status().await,
        EmergencyShortcutStatus::Unavailable
    );
    assert!(fixture.activations.try_recv().is_err());
}

#[tokio::test]
async fn revoked_session_ends_monitoring() {
    let mut fixture = Fixture::new(0).await;
    let _ = fixture.next_status().await;
    fixture
        .server
        .emit_signal(
            None::<&str>,
            fixture.session_path(),
            "org.freedesktop.portal.Session",
            "Closed",
            &Options::new(),
        )
        .await
        .expect("revocation");
    let result = tokio::time::timeout(Duration::from_secs(5), &mut fixture.task)
        .await
        .expect("revocation timeout")
        .expect("task");
    assert!(result.is_err());
    assert_eq!(
        fixture.next_status().await,
        EmergencyShortcutStatus::Unavailable
    );
}

#[tokio::test]
async fn portal_exit_invalidates_the_registered_shortcut() {
    let mut fixture = Fixture::new(0).await;
    let _ = fixture.next_status().await;
    fixture
        .server
        .release_name("org.freedesktop.portal.Desktop")
        .await
        .expect("release portal name");
    assert_eq!(
        fixture.next_status().await,
        EmergencyShortcutStatus::Unavailable
    );
}
