const BUS_NAME = "uk.tvidal";
const OBJECT_PATH = "/WindowManager";
const INTERFACE = "uk.tvidal.KDETool";

const sendReply = (reply) => callDBus(
    BUS_NAME,
    OBJECT_PATH,
    INTERFACE,
    "sendReply",
    reply
);

const processAction = (action) => {
    print(`kdetool: ${action}`);
    sendReply("OK");
};

const fetchNextAction = () => callDBus(
    BUS_NAME,
    OBJECT_PATH,
    INTERFACE,
    "fetchNextAction",
    processAction
);

registerShortcut("kdetoolAction", "Triggers a KDETool action", null, fetchNextAction);
print("kdetool: KWin script loaded");
