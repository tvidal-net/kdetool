use crate::TIMEOUT;
use dbus::Error;
use dbus::blocking::{Connection, Proxy};

const KWIN_BUS_NAME: &str = "org.kde.KWin";

const KWIN_SCRIPTING_PATH: &str = "/Scripting";
const KWIN_SCRIPTING_INTERFACE: &str = "org.kde.kwin.Scripting";
const KWIN_IS_SCRIPT_LOADED: &str = "isScriptLoaded";

const KWIN_COMPONENT_PATH: &str = "/component/kwin";
const KWIN_COMPONENT_INTERFACE: &str = "org.kde.kglobalaccel.Component";
const KWIN_INVOKE_SHORTCUT: &str = "invokeShortcut";

// Identifiers of the bundled KWin script and the shortcuts it registers, kept in
// sync with the constants at the top of kwin/contents/code/main.js.
const KWIN_PLUGIN: &str = "KWinTool";
const KWIN_SHORTCUT_ACTION: &str = "KWinToolAction";
const KWIN_SHORTCUT_RECONFIGURE: &str = "KWinToolReconfigure";

pub trait KWin {
    /// Reports whether the bundled KWin script is currently loaded.
    fn is_script_loaded(&self) -> Result<bool, Error>;
    /// Triggers the focus-or-start shortcut, making the script call back into the
    /// one-shot client (fetchNextAction/sendReply).
    fn invoke_shortcut(&self) -> Result<(), Error>;
    /// Triggers the reconfigure shortcut, making the script re-fetch its target
    /// list from the background service (GetTargets).
    fn invoke_reconfigure(&self) -> Result<(), Error>;
}

pub struct KWinClient {
    connection: Connection,
}

impl KWinClient {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            connection: Connection::new_session()?,
        })
    }

    fn dbus<'a>(&self, path: &'a str) -> Proxy<'a, &Connection> {
        Proxy::new(KWIN_BUS_NAME, path, TIMEOUT, &self.connection)
    }
}

impl KWin for KWinClient {
    fn is_script_loaded(&self) -> Result<bool, Error> {
        self.dbus(KWIN_SCRIPTING_PATH)
            .method_call(
                KWIN_SCRIPTING_INTERFACE,
                KWIN_IS_SCRIPT_LOADED,
                (KWIN_PLUGIN,),
            )
            .map(|(loaded,): (bool,)| loaded)
    }

    fn invoke_shortcut(&self) -> Result<(), Error> {
        self.invoke(KWIN_SHORTCUT_ACTION)
    }

    fn invoke_reconfigure(&self) -> Result<(), Error> {
        self.invoke(KWIN_SHORTCUT_RECONFIGURE)
    }
}

impl KWinClient {
    fn invoke(&self, shortcut: &str) -> Result<(), Error> {
        self.dbus(KWIN_COMPONENT_PATH).method_call(
            KWIN_COMPONENT_INTERFACE,
            KWIN_INVOKE_SHORTCUT,
            (shortcut,),
        )
    }
}
