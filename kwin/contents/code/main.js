const SERVICE = "uk.tvidal";
const PATH = "/WindowManager";
const INTERFACE = "uk.tvidal.KDETool";

function processAction(action) {
    print(`kdetool: ${action}`);
}

const fetchNextAction = () => callDBus(
    SERVICE,
    PATH,
    INTERFACE,
    "fetchNextAction",
    [],
    processAction
);

registerShortcut("kdetoolAction", "Triggers a KDETool action", null, fetchNextAction);
print("kdetool: KWin script loaded");
