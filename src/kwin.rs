use crate::TIMEOUT;
use dbus;
use dbus::blocking::{Connection, Proxy};
use dbus::Error;
use std::path;

const KWIN_BUS_NAME: &str = "org.kde.KWin";
const KWIN_PLUGIN: &str = "kwintool";
const KWIN_SHORTCUT: &str = "kdetoolAction";
const KWIN_SCRIPTING: &str = "org.kde.kwin.Scripting";

pub trait KWin {
    fn invoke_shortcut(&self) -> Result<(), Error>;
    fn is_script_loaded(&self) -> Result<bool, Error>;
    fn load_script(&self, path: &path::Path) -> Result<i32, Error>;
    fn start(&self) -> Result<(), Error>;
}

pub struct KWinClient {
    connection: Connection,
}

impl KWinClient {
    pub fn new() -> Self {
        Self {
            connection: Connection::new_session().unwrap(),
        }
    }

    fn dbus<'a>(&self, path: &'a str) -> Proxy<'a, &Connection> {
        Proxy::new(KWIN_BUS_NAME, path, TIMEOUT, &self.connection)
    }

    fn scripting<'a>(&self) -> Proxy<'a, &Connection> {
        self.dbus("/Scripting")
    }

    fn component<'a>(&self) -> Proxy<'a, &Connection> {
        self.dbus("/component/kwin")
    }
}

impl KWin for KWinClient {
    fn invoke_shortcut(&self) -> Result<(), Error> {
        let dbus = self.component();
        dbus.method_call(
            "org.kde.kglobalaccel.Component",
            "invokeShortcut",
            (KWIN_SHORTCUT,),
        )
    }

    fn is_script_loaded(&self) -> Result<bool, Error> {
        let dbus = self.scripting();
        dbus.method_call("org.kde.kwin.Scripting", "isScriptLoaded", (KWIN_PLUGIN,))
            .map(|(reply,)| reply)
    }

    fn load_script(&self, path: &path::Path) -> Result<i32, Error> {
        let dbus = self.scripting();
        let path = path.to_str().unwrap();
        dbus.method_call("org.kde.kwin.Scripting", "loadScript", (path, KWIN_PLUGIN))
            .map(|(reply,)| reply)
    }

    fn start(&self) -> Result<(), Error> {
        let dbus = self.scripting();
        dbus.method_call(KWIN_SCRIPTING, "start", ())
    }
}
