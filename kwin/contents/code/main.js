const BUS_NAME = "uk.tvidal";
const OBJECT_PATH = "/KWinTool";
const INTERFACE = "uk.tvidal.KWinTool";

const dbus = (methodName, ...args) => callDBus(
    BUS_NAME,
    OBJECT_PATH,
    INTERFACE,
    methodName,
    ...args
);

const sendReply = (reply) => dbus(
    "sendReply",
    reply
);

const processAction = (action) => {
    print(`KWinTool: ${action}`);
    sendReply("OK");
};

const fetchNextAction = () => dbus(
    "fetchNextAction",
    processAction
);

registerShortcut("KWinToolAction", "Triggers a KWinTool action", null, fetchNextAction);
print("KWinTool: KWin Script Loaded");
