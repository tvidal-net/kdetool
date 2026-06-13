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

// Focuses the window whose resourceClass matches the requested pattern. When
// the active window already matches, the next matching window in the stacking
// order is focused instead, so repeated calls cycle through them.
const activate = (action) => {
    const className = new RegExp(action.class, "i");
    const matches = (win) => win && win.normalWindow && className.test(win.resourceClass);
    const windows = workspace.stackingOrder.filter(matches);
    if (windows.length === 0) {
        return "not-found";
    }
    workspace.activeWindow = windows[matches(workspace.activeWindow) ? 0 : windows.length - 1];
    return "activated";
};

const processAction = (json) => {
    let status;
    try {
        const action = JSON.parse(json);
        status = action.type === "activate" ? activate(action) : "ok";
    } catch (error) {
        status = `error: ${error}`;
    }
    print(`KWinTool: ${json} -> ${status}`);
    sendReply(status);
};

const fetchNextAction = () => dbus(
    "fetchNextAction",
    processAction
);

registerShortcut("KWinToolAction", "Triggers a KWinTool action", null, fetchNextAction);
print("KWinTool: KWin Script Loaded");
