const BUS_NAME = "uk.tvidal";
const OBJECT_PATH = "/WindowManager";
const INTERFACE = "uk.tvidal.KWinTool";

const sendReply = (reply) => callDBus(
    BUS_NAME,
    OBJECT_PATH,
    INTERFACE,
    "sendReply",
    reply
);

const processAction = (action) => {
    print(`KWinTool: ${action}`);
    sendReply("OK");
};

const fetchNextAction = () => callDBus(
    BUS_NAME,
    OBJECT_PATH,
    INTERFACE,
    "fetchNextAction",
    processAction
);

registerShortcut("kdetoolAction", "Triggers a KWinTool action", null, fetchNextAction);
print("KWinTool: KWin script loaded");
