use crate::TIMEOUT;
use dbus;
use dbus::blocking::{Connection, Proxy};
use dbus::Error;
use std::path;

const KWIN_BUS_NAME: &str = "org.kde.KWin";

const KWIN_SCRIPTING: &str = "/Scripting";
const KWIN_IS_SCRIPT_LOADED: &str = "org.kde.kwin.Scripting.isScriptLoaded";
const KWIN_LOAD_SCRIPT: &str = "org.kde.kwin.Scripting.loadScript";
const KWIN_START: &str = "org.kde.kwin.Scripting.start";

const KWIN_COMPONENT: &str = "/component/kwin";
const KWIN_INVOKE_SHORTCUT: &str = "org.kde.kglobalaccel.Component.invokeShortcut";

const KWIN_PATH: &str = "/KWin";
const KWIN_QUERY_WINDOW_INFO: &str = "org.kde.kwin.KWin.queryWindowInfo";

const KWIN_PLUGIN: &str = "KWinTool";
const KWIN_SHORTCUT: &str = "KWinToolAction";

pub trait KWin {
    fn is_script_loaded(&self) -> Result<bool, Error>;
    fn load_script(&self, path: &path::Path) -> Result<i32, Error>;
    fn start(&self) -> Result<(), Error>;
    fn invoke_shortcut(&self) -> Result<(), Error>;
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
        self.dbus(KWIN_SCRIPTING)
    }

    fn component<'a>(&self) -> Proxy<'a, &Connection> {
        self.dbus(KWIN_COMPONENT)
    }

    fn kwin<'a>(&self) -> Proxy<'a, &Connection> {
        self.dbus(KWIN_PATH)
    }
}

impl KWin for KWinClient {
    fn is_script_loaded(&self) -> Result<bool, Error> {
        self.scripting()
            .method_call(KWIN_SCRIPTING, KWIN_IS_SCRIPT_LOADED, (KWIN_PLUGIN,))
            .map(|(reply,)| reply)
    }

    fn load_script(&self, path: &path::Path) -> Result<i32, Error> {
        let path = path.to_str().unwrap();
        self.scripting()
            .method_call(KWIN_SCRIPTING, KWIN_LOAD_SCRIPT, (path, KWIN_PLUGIN))
            .map(|(reply,)| reply)
    }

    fn start(&self) -> Result<(), Error> {
        self.scripting()
            .method_call(KWIN_SCRIPTING, KWIN_START, ())
    }

    fn invoke_shortcut(&self) -> Result<(), Error> {
        self.component()
            .method_call(KWIN_COMPONENT, KWIN_INVOKE_SHORTCUT, (KWIN_SHORTCUT,))
    }
}
