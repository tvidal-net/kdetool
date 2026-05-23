use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dbus::Error;
use dbus::blocking::Connection;
use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus_crossroads::{Crossroads, MethodErr};

// DBus identity of the tool, kept in sync with the SERVICE/PATH/INTERFACE
// constants at the top of kwin/contents/code/main.js.
const BUS_NAME: &str = "uk.tvidal.KDETool";
const OBJECT_PATH: &str = "/Windows";
const INTERFACE: &str = "uk.tvidal.WindowManager";

// KWin exposes script-registered shortcuts through its kglobalaccel component;
// invoking one is how we wake the persistent script without binding a key.
const KGLOBALACCEL_SERVICE: &str = "org.kde.kglobalaccel";
const KGLOBALACCEL_PATH: &str = "/component/kwin";
const KGLOBALACCEL_INTERFACE: &str = "org.kde.kglobalaccel.Component";
const SHORTCUT_NAME: &str = "kdetoolAction";

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub fn run() -> Result<(), Error> {
    let connection = Connection::new_session()?;
    match connection.request_name(BUS_NAME, false, true, false)? {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {}
        reply => {
            return Err(Error::new_custom(
                "uk.tvidal.KDETool.NameNotAcquired",
                &format!("could not acquire {BUS_NAME}: {reply:?}"),
            ));
        }
    }

    let stop = Arc::new(AtomicBool::new(false));
    let stop_handler = Arc::clone(&stop);

    let mut crossroads = Crossroads::new();
    let interface = crossroads.register(INTERFACE, |builder| {
        builder.method(
            "fetchNextAction",
            (),
            ("action",),
            move |_, _, _: ()| -> Result<(String,), MethodErr> {
                stop_handler.store(true, Ordering::SeqCst);
                Ok(("Hello World".to_string(),))
            },
        );
    });
    crossroads.insert(OBJECT_PATH, &[interface], ());

    connection.start_receive(
        MatchRule::new_method_call(),
        Box::new(move |msg, conn| crossroads.handle_message(msg, conn).is_ok()),
    );

    // Wake the KWin script: its shortcut callback calls fetchNextAction back on
    // us. The name is already owned above, so that callDBus target resolves.
    let kglobalaccel =
        connection.with_proxy(KGLOBALACCEL_SERVICE, KGLOBALACCEL_PATH, DEFAULT_TIMEOUT);
    let _: () =
        kglobalaccel.method_call(KGLOBALACCEL_INTERFACE, "invokeShortcut", (SHORTCUT_NAME,))?;

    // Serve method calls until fetchNextAction has been answered. The handler
    // sets the flag after its reply is queued, so the next iteration exits with
    // "Hello World" already on its way back to the script.
    while !stop.load(Ordering::SeqCst) {
        connection.process(POLL_INTERVAL)?;
    }

    connection.release_name(BUS_NAME)?;
    Ok(())
}
