use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::TIMEOUT;

use dbus::Error;
use dbus::blocking::Connection;
use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus_crossroads::{Crossroads, MethodErr};

// DBus identity of the one-shot client, kept in sync with the CLIENT_* constants
// at the top of kwin/contents/code/main.js. The transient `kwintool <app>`
// process owns this name only while it drives a focus-or-start round-trip; the
// systemd-activated background service owns a separate name (see server.rs).
const BUS_NAME: &str = "uk.tvidal.client";
const OBJECT_PATH: &str = "/KWinTool";
const INTERFACE: &str = "uk.tvidal.client";

const POLL_INTERVAL: Duration = Duration::from_millis(500);

// Two concurrent `kwintool <app>` invocations contend for the single client
// name. Rather than replacing (and interrupting) an in-flight round-trip, a
// second invocation waits for the first to finish, retrying for up to 10s.
const NAME_RETRY_INTERVAL: Duration = Duration::from_secs(1);
const NAME_RETRY_ATTEMPTS: u32 = 10;

// Requests the well-known client name, waiting out any other client that
// currently owns it. `do_not_queue` makes a taken name report immediately so we
// control the retry cadence ourselves instead of blocking in the daemon queue.
fn acquire_name(connection: &Connection) -> Result<(), Error> {
    for attempt in 1..=NAME_RETRY_ATTEMPTS {
        match connection.request_name(BUS_NAME, false, false, true)? {
            RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => return Ok(()),
            reply if attempt < NAME_RETRY_ATTEMPTS => {
                let _ = reply;
                sleep(NAME_RETRY_INTERVAL);
            }
            reply => {
                return Err(Error::new_custom(
                    "uk.tvidal.client.NameNotAcquired",
                    &format!(
                        "could not acquire {BUS_NAME} after {NAME_RETRY_ATTEMPTS} attempts: {reply:?}"
                    ),
                ));
            }
        }
    }
    unreachable!("loop returns on the final attempt")
}

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
        acquire_name(&connection)?;

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
