use std::time::Duration;

use dbus::blocking::{Connection, Proxy};

const KWIN_SERVICE: &str = "org.kde.KWin";
const KWIN_PATH: &str = "/KWin";
const KWIN_INTERFACE: &str = "org.kde.KWin";

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct KWinClient {
    connection: Connection,
}

impl KWinClient {
    pub fn new() -> Result<Self, dbus::Error> {
        let connection = Connection::new_session()?;
        Ok(Self { connection })
    }

    fn proxy(&self) -> Proxy<'_, &Connection> {
        self.connection
            .with_proxy(KWIN_SERVICE, KWIN_PATH, DEFAULT_TIMEOUT)
    }

    pub fn active_output_name(&self) -> Result<String, dbus::Error> {
        let (name,): (String,) =
            self.proxy()
                .method_call(KWIN_INTERFACE, "activeOutputName", ())?;
        Ok(name)
    }
}
