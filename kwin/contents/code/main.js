const BUS_NAME = "uk.tvidal";
const OBJECT_PATH = "/WindowManager";
const INTERFACE = "uk.tvidal.KDETool";

const processAction = (action) => {
    print(`kdetool: ${action}`);
}

const fetchNextAction = () => callDBus(
    BUS_NAME,
    OBJECT_PATH,
    INTERFACE,
    "fetchNextAction",
    [],
    processAction
);

registerShortcut("kdetoolAction", "Triggers a KDETool action", null, fetchNextAction);
print("kdetool: KWin script loaded");
