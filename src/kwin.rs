use crate::TIMEOUT;
use dbus::Error;
use dbus::blocking::Connection;
use std::path::Path;

const KWIN_BUS_NAME: &str = "org.kde.KWin";

const KWIN_SCRIPTING_PATH: &str = "/Scripting";
const KWIN_SCRIPTING_INTERFACE: &str = "org.kde.kwin.Scripting";
const KWIN_LOAD_SCRIPT: &str = "loadScript";
const KWIN_START: &str = "start";
const KWIN_PLUGIN_NAME: &str = "kdetool";

const KWIN_COMPONENT_PATH: &str = "/component/kwin";
const KWIN_COMPONENT_INTERFACE: &str = "org.kde.kglobalaccel.Component";
const KWIN_INVOKE_SHORTCUT: &str = "invokeShortcut";

pub trait KWin {
    fn invoke_shortcut(&self, name: &str) -> Option<dbus::Error>;
    fn load_script(&self, path: &Path) -> Result<u32, dbus::Error>;
    fn start(&self) -> Option<dbus::Error>;
}

pub struct KWinClient {
    connection: Connection,
}

impl KWinClient {
    pub fn new() -> Result<Self, dbus::Error> {
        Ok(Self {
            connection: Connection::new_session()?,
        })
    }
}

impl KWin for KWinClient {
    fn invoke_shortcut(&self, name: &str) -> Option<Error> {
        let component = self
            .connection
            .with_proxy(KWIN_BUS_NAME, KWIN_COMPONENT_PATH, TIMEOUT);
        let reply: Result<(), Error> =
            component.method_call(KWIN_COMPONENT_INTERFACE, KWIN_INVOKE_SHORTCUT, (name,));
        reply.err()
    }

    fn load_script(&self, path: &Path) -> Result<u32, Error> {
        let path = path.to_str().ok_or_else(|| {
            Error::new_custom("uk.tvidal.kdetool.InvalidPath", "script path is not valid UTF-8")
        })?;
        let scripting = self
            .connection
            .with_proxy(KWIN_BUS_NAME, KWIN_SCRIPTING_PATH, TIMEOUT);
        let (id,): (i32,) = scripting.method_call(
            KWIN_SCRIPTING_INTERFACE,
            KWIN_LOAD_SCRIPT,
            (path, KWIN_PLUGIN_NAME),
        )?;
        // loadScript returns the new script id, or a negative value on failure
        // (already loaded under KWIN_PLUGIN_NAME, or the file could not be
        // read); surface that as an error.
        u32::try_from(id).map_err(|_| {
            Error::new_custom(
                "uk.tvidal.kdetool.LoadScriptFailed",
                &format!("loadScript returned {id}"),
            )
        })
    }

    fn start(&self) -> Option<Error> {
        let scripting = self
            .connection
            .with_proxy(KWIN_BUS_NAME, KWIN_SCRIPTING_PATH, TIMEOUT);
        let reply: Result<(), Error> =
            scripting.method_call(KWIN_SCRIPTING_INTERFACE, KWIN_START, ());
        reply.err()
    }
}
