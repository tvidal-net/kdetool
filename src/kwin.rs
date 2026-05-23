use dbus::blocking::Connection;
use std::path::Path;
use dbus::Error;

const KWIN_BUS_NAME: &str = "org.kde.KWin";

const KWIN_SCRIPTING_PATH: &str = "/Scripting";
const KWIN_SCRIPTING_INTERFACE: &str = "org.kde.kwin.Scripting";
const KWIN_LOAD_SCRIPT: &str = "loadScript";
const KWIN_START: &str = "start";

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
        todo!("callDBus()")
    }

    fn load_script(&self, path: &Path) -> Result<u32, Error> {
        todo!("callDBus()")
    }

    fn start(&self) -> Option<Error> {
        todo!("callDBus()")
    }
}
