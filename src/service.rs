use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
use dbus::blocking::Connection;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus::Error;
use dbus_crossroads::{Crossroads, MethodErr};

// DBus identity of the tool, kept in sync with the constants at the top of
// kwin/contents/code/main.js.
const BUS_NAME: &str = "uk.tvidal";
const OBJECT_PATH: &str = "/KWinTool";
const INTERFACE: &str = "uk.tvidal.KWinTool";

const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub struct Service {
    connection: Connection,
    stop: Arc<AtomicBool>,
}

fn fetch_next_action() -> Result<(String,), MethodErr> {
    Ok(("Hello World".to_string(),))
}

fn send_reply(args: (String,)) -> Result<(), MethodErr> {
    let (reply,) = args;
    println!("Reply: {}", reply);
    Ok(())
}

impl Service {
    /// Owns the well-known name and registers the fetchNextAction and sendReply
    /// methods, but does not process anything yet. The caller is expected to
    /// trigger the KWin script (invokeShortcut) before calling [`serve`], so the
    /// script's callbacks resolve against an already-owned name; they queue on
    /// the connection until the serve loop processes them.
    pub fn register() -> Result<Self, Error> {
        let connection = Connection::new_session()?;
        match connection.request_name(BUS_NAME, false, true, false)? {
            RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {}
            reply => {
                return Err(Error::new_custom(
                    "uk.tvidal.KWinTool.NameNotAcquired",
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
                |_, _, _: ()| -> Result<(String,), MethodErr> { Ok(("Hello World".to_string(),)) },
            );
            builder.method(
                "sendReply",
                ("reply",),
                (),
                move |_, _, (reply,): (String,)| {
                    stop_handler.store(true, Ordering::SeqCst);
                    println!("Received reply: {}", reply);
                    Ok(())
                },
            );
        });
        crossroads.insert(OBJECT_PATH, &[interface], ());

        connection.start_receive(
            MatchRule::new_method_call(),
            Box::new(move |msg, conn| crossroads.handle_message(msg, conn).is_ok()),
        );

        Ok(Self { connection, stop })
    }

    /// Processes incoming method calls until sendReply flips the stop flag, then
    /// drops the well-known name. The connection (and the registered object) is
    /// released when the returned value goes out of scope.
    pub fn serve(self) -> Result<(), Error> {
        while !self.stop.load(Ordering::SeqCst) {
            self.connection.process(POLL_INTERVAL)?;
        }
        self.connection.release_name(BUS_NAME)?;
        Ok(())
    }
}
