use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::TIMEOUT;

use dbus::Error;
use dbus::blocking::Connection;
use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
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
    reply: Arc<Mutex<Option<String>>>,
}

impl Service {
    /// Owns the well-known name and registers the fetchNextAction and sendReply
    /// methods, but does not process anything yet. `action` is the payload handed
    /// to the script when it calls fetchNextAction. The caller is expected to
    /// trigger the KWin script (invokeShortcut) before calling [`serve`], so the
    /// script's callbacks resolve against an already-owned name; they queue on
    /// the connection until the serve loop processes them.
    pub fn register(action: String) -> Result<Self, Error> {
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
        let reply = Arc::new(Mutex::new(None));

        let stop_handler = Arc::clone(&stop);
        let reply_slot = Arc::clone(&reply);

        let mut crossroads = Crossroads::new();
        let interface = crossroads.register(INTERFACE, move |builder| {
            builder.method(
                "fetchNextAction",
                (),
                ("action",),
                move |_, _, _: ()| -> Result<(String,), MethodErr> { Ok((action.clone(),)) },
            );
            builder.method(
                "sendReply",
                ("reply",),
                (),
                move |_, _, (reply,): (String,)| {
                    *reply_slot.lock().expect("reply mutex poisoned") = Some(reply);
                    stop_handler.store(true, Ordering::SeqCst);
                    Ok(())
                },
            );
        });
        crossroads.insert(OBJECT_PATH, &[interface], ());

        connection.start_receive(
            MatchRule::new_method_call(),
            Box::new(move |msg, conn| crossroads.handle_message(msg, conn).is_ok()),
        );

        Ok(Self {
            connection,
            stop,
            reply,
        })
    }

    /// Processes incoming method calls until sendReply flips the stop flag or
    /// [`TIMEOUT`] elapses, then drops the well-known name and returns the status
    /// the script reported. A `None` result means the script never replied within
    /// the timeout.
    pub fn serve(self) -> Result<Option<String>, Error> {
        let deadline = Instant::now() + TIMEOUT;
        while !self.stop.load(Ordering::SeqCst) {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            self.connection
                .process((deadline - now).min(POLL_INTERVAL))?;
        }
        self.connection.release_name(BUS_NAME)?;
        Ok(self
            .reply
            .lock()
            .expect("reply mutex poisoned")
            .take())
    }
}
