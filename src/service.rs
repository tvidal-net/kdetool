use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dbus::Error;
use dbus::blocking::Connection;
use dbus::blocking::stdintf::org_freedesktop_dbus::RequestNameReply;
use dbus::channel::{MatchingReceiver, Sender};
use dbus::message::{MatchRule, Message};
use dbus_crossroads::{Crossroads, MethodErr};

const BUS_NAME: &str = "uk.tvidal.kdetool";
const OBJECT_PATH: &str = "/uk/tvidal/kdetool";
const INTERFACE: &str = "uk.tvidal.kdetool";

const POLL_INTERVAL: Duration = Duration::from_millis(500);

type Window = (String, String, String);

pub fn query() -> Result<(), Error> {
    let connection = Connection::new_session()?;
    match connection.request_name(BUS_NAME, false, true, false)? {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => {}
        reply => {
            return Err(Error::new_custom(
                "uk.tvidal.kdetool.NameNotAcquired",
                &format!("could not acquire {BUS_NAME}: {reply:?}"),
            ));
        }
    }

    let stop = Arc::new(AtomicBool::new(false));
    let stop_handler = Arc::clone(&stop);

    let mut crossroads = Crossroads::new();
    let interface = crossroads.register(INTERFACE, |builder| {
        builder.signal::<(), _>("query", ());
        builder.method(
            "windows",
            ("records",),
            (),
            move |_, _, (records,): (Vec<Window>,)| -> Result<(), MethodErr> {
                for (id, resource_class, name) in records {
                    println!("{id}\t{resource_class}\t{name}");
                }
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

    let signal = Message::new_signal(OBJECT_PATH, INTERFACE, "query")
        .map_err(|err| Error::new_custom("uk.tvidal.kdetool.SignalError", &err))?;
    connection.send(signal).map_err(|_| {
        Error::new_custom("uk.tvidal.kdetool.SignalError", "failed to emit query signal")
    })?;

    while !stop.load(Ordering::SeqCst) {
        connection.process(POLL_INTERVAL)?;
    }

    Ok(())
}
