use std::process::exit;

use dbus::blocking::Connection;
use dbus_crossroads::{Crossroads, MethodErr};

const BUS_NAME: &str = "uk.tvidal.kdetool";
const OBJECT_PATH: &str = "/uk/tvidal/kdetool";
const INTERFACE: &str = "uk.tvidal.kdetool";

pub fn serve() -> Result<(), dbus::Error> {
    let connection = Connection::new_session()?;
    connection.request_name(BUS_NAME, false, true, false)?;

    let mut crossroads = Crossroads::new();
    let interface = crossroads.register(INTERFACE, |builder| {
        builder.method("ping", (), (), |_, _, (): ()| -> Result<(), MethodErr> {
            println!("ping");
            exit(0);
        });
    });
    crossroads.insert(OBJECT_PATH, &[interface], ());

    crossroads.serve(&connection)
}
