use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use dbus::blocking::Connection;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus_crossroads::{Crossroads, MethodErr};

const BUS_NAME: &str = "uk.tvidal.kdetool";
const OBJECT_PATH: &str = "/uk/tvidal/kdetool";
const INTERFACE: &str = "uk.tvidal.kdetool";

const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub fn serve() -> Result<(), dbus::Error> {
    let connection = Connection::new_session()?;
    connection.request_name(BUS_NAME, false, true, false)?;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_handler = Arc::clone(&stop);

    let mut crossroads = Crossroads::new();
    let interface = crossroads.register(INTERFACE, |builder| {
        builder.method(
            "ping",
            (),
            (),
            move |_, _, (): ()| -> Result<(), MethodErr> {
                println!("ping");
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

    while !stop.load(Ordering::SeqCst) {
        connection.process(POLL_INTERVAL)?;
    }

    Ok(())
}
