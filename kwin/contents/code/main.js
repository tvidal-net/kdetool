// One-shot client: the transient `kwintool <app>` process that drives a
// focus-or-start round-trip (fetchNextAction/sendReply). Kept in sync with the
// constants in src/service.rs.
const BUS_NAME = "uk.tvidal.client";
const OBJECT_PATH = "/KWinTool";
const INTERFACE = "uk.tvidal.client";

const WIN_NORMAL = 0;
const REGEX_FLAGS = "i";
const CH_NOT = "!";

const SEP_SEARCH = "&&";
const SEP_ACTION = ";";
const SEP_VALUE = "=";

const RE_NAME = /^[a-z]+/i;
const RE_GEO = /([xywhvm])([0-9%]+)?/gi

const DELAY_MS = 50;

let debugEnabled = false;

function logError(err) {
    console.log(`=> ERROR: ${err}`);
}

function logDebug(win, s) {
    if (debugEnabled) {
        console.log(`{${win.resourceName}:${win.caption}} => ${s}`);
    }
}

function parse(str) {
    const [name, value] = str.split(SEP_VALUE, 2);
    const match = name[name.length - 1] !== CH_NOT;
    return [
        match ? name : name.substring(0, name.length - 1),
        value,
        match
    ]
}

class SimpleMatch {

    constructor(str, getValue) {
        const [name, value, match] = parse(str);
        this.name = name;
        this.value = value;
        this.match = match;
        this.getValue = getValue;
    }

    matches(win) {
        const winValue = this.getValue(win)
        if (typeof winValue === "number") {
            return (winValue === parseInt(this.value)) === this.match;
        }
        return (`${winValue}` === `${this.value}`) === this.match;
    }

    toString() {
        return `${this.name} ${this.match ? "" : CH_NOT}== ${this.value}`
    }
}

class RegExpMatch {

    constructor(str, getText) {
        const [name, pattern, match] = parse(str);
        this.name = name
        this.match = match;
        this.regexp = new RegExp(pattern, REGEX_FLAGS);
        this.getText = getText;
    }

    matches(win) {
        return this.regexp.test(this.getText(win)) === this.match;
    }

    toString() {
        return `${this.name} ${this.match ? "" : CH_NOT}= /${this.regexp.source}/${this.regexp.flags}`;
    }
}

function searchMatch(search) {
    const name = search.match(RE_NAME).toString();
    switch (name) {
        case "id":
            return new SimpleMatch(search, w => w.internalId);
        case "role":
            return new SimpleMatch(search, w => w.windowRole);
        case "pid":
            return new SimpleMatch(search, w => w.pid);
        case "desktop":
            // Match the same 0-based index into workspace.desktops that
            // MoveToDesktopAction uses, so `-d N` and `-D N` mean the same
            // desktop. A window on all desktops (empty list) matches none.
            return new SimpleMatch(search, w =>
                w.desktops && w.desktops.length
                    ? workspace.desktops.indexOf(w.desktops[0])
                    : -1);
        case "name":
            return new RegExpMatch(search, w => w.resourceName);
        case "class":
            return new RegExpMatch(search, w => w.resourceClass);
        case "title":
            // Match against "caption:resourceClass" so patterns can anchor on
            // the class (e.g. `-fleet$` or `^:jetbrains-`), matching how the Rust
            // config and CLI treat title.
            return new RegExpMatch(search, w => `${w.caption}:${w.resourceClass}`);
        default:
            throw `invalid search criteria: ${search}`
    }
}

class MoveToDesktopAction {

    constructor(desktop) {
        this.desktop = parseInt(desktop);
    }

    execute(win) {
        logDebug(win, this);
        if (this.desktop < 0) {
            win.desktops = [];
            win.onAllDesktops = true;
        } else {
            const desktop = this.desktop >= workspace.desktops.length
                ? workspace.currentDesktop
                : workspace.desktops[this.desktop];

            win.desktops = [desktop];
        }
    }

    toString() {
        return `ToDesktop = ${this.desktop}`;
    }
}

class MoveToScreenAction {

    constructor(screen) {
        this.screen = new RegExp(screen, REGEX_FLAGS);
    }

    execute(win) {
        // Match the connector name (e.g. "DP-2", "HDMI-A-1"), not the EDID model
        // string: `-S DP` means the DisplayPort output, which lives in `name`.
        const screen = workspace.screens
            .find(s => this.screen.test(s.name));

        if (screen) {
            logDebug(win, `${this} (${screen.manufacturer} ${screen.name} ${screen.model})`);
            return workspace.sendClientToScreen(win, screen);
        }
        throw `Screen not found: /${this.screen.source}/${this.screen.flags}`;
    }

    toString() {
        return `ToScreen = /${this.screen.source}/${this.screen.flags}`;
    }
}

class NoBorderAction {

    constructor(value) {
        this.noBorder = value === "true";
    }

    execute(win) {
        logDebug(win, this);
        win.noBorder = this.noBorder;
    }

    toString() {
        return `NoBorder = ${this.noBorder}`;
    }
}

class MaximizeAction {

    constructor(value) {
        this.maximize = value === "true";
    }

    execute(win) {
        logDebug(win, this);
        win.setMaximize(this.maximize, this.maximize);
    }

    toString() {
        return `Maximize = ${this.maximize}`;
    }
}

class KeepBelowAction {

    constructor(value) {
        this.keepBelow = value === "true";
    }

    execute(win) {
        logDebug(win, this);
        win.keepBelow = this.keepBelow;
    }

    toString() {
        return `KeepBelow = ${this.keepBelow}`;
    }
}

function computeGeometry(min, max, value) {
    const n = parseFloat(value);
    return min + (/%/.test(value) ? n * max / 100.0 : n);
}

class GeometryAction {

    constructor(geometry) {
        this.geometry = {};
        let match = [];
        while (match = RE_GEO.exec(geometry)) {
            this.geometry[match[1].toLowerCase()] = match[2] ?? true;
        }
    }

    execute(win) {
        const timer = new QTimer();
        timer.singleShot = true;
        timer.interval = DELAY_MS;
        timer.timeout.connect(() => {
            const a = workspace.clientArea(KWin.MaximizeArea, win);
            let geo = {};
            for (let ch in this.geometry) {
                const value = this.geometry[ch];
                switch (ch) {
                    // Positions are offset by the area origin (so a screen at
                    // left/top > 0 places correctly); sizes are proportional to
                    // the area only — offsetting them by the origin makes a
                    // window on a non-primary screen grow far too wide/tall.
                    case "x":
                        geo.x = computeGeometry(a.left, a.width, value);
                        break;
                    case "y":
                        geo.y = computeGeometry(a.top, a.height, value);
                        break;
                    case "w":
                        geo.width = computeGeometry(0, a.width, value);
                        break;
                    case "h":
                        geo.height = computeGeometry(0, a.height, value);
                        break;
                }
            }
            win.frameGeometry = Object.assign({}, win.frameGeometry, geo);
            // Maximize LAST, once the target size is in place: vertical/both
            // maximize then stretches from the size we just set. Doing it before
            // the resize leaves the window stuck at its old (wide) width.
            win.setMaximize(!!this.geometry["m"] || !!this.geometry["v"], !!this.geometry["m"]);
            logDebug(win, `Geometry ${this} => ${JSON.stringify(geo)}`);
        });
        timer.start();
    }

    toString() {
        return JSON.stringify(this.geometry);
    }
}

class ActivateAction {
    execute(win) {
        logDebug(win, this);
        workspace.activeWindow = win;
    }

    toString() {
        return "Activate";
    }
}

function windowAction(action) {
    const [name, value] = parse(action);
    switch (name) {
        case "screen":
            return new MoveToScreenAction(value);
        case "desktop":
            return new MoveToDesktopAction(value);
        case "noborder":
            return new NoBorderAction(value);
        case "maximize":
            return new MaximizeAction(value);
        case "keepbelow":
            return new KeepBelowAction(value);
        case "geometry":
            return new GeometryAction(value);
        case "activate":
            return new ActivateAction();
        default:
            throw `invalid window action: ${action}`;
    }
}

class WindowAction {
    constructor(str) {
        const parts = str.split(SEP_SEARCH);
        this.search = parts.slice(0, -1)
            .map(searchMatch);

        const actions = parts.slice(-1);
        this.actions = actions[0].split(SEP_ACTION)
            .filter(a => !!a)
            .map(windowAction);
    }

    matches(win) {
        return win.windowType === WIN_NORMAL
            && this.search.every(s => s.matches(win));
    }

    run() {
        const windows = workspace.stackingOrder
            .filter(w => this.matches(w));

        // No window matches the search at all: report NotFound so the tool can
        // start the program. A match that merely happens to be the active
        // window must NOT reach here, or focus-or-start would spawn a duplicate.
        if (windows.length === 0) {
            return null;
        }

        // Prefer a match other than the active window so repeated invocations
        // cycle through the matches; if the active window is the only match,
        // act on it (activating it is a no-op but still confirms it exists).
        const win = windows.find(w => w !== workspace.activeWindow) ?? windows[0];
        logDebug(win, `Matches ${this}`);
        this.actions.forEach(a => a.execute(win));
        return win.internalId;
    }

    toString() {
        const search = this.search.map(s => s.toString());
        return `{${search.join(", ")}}`;
    }
}

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

const processAction = (str) => {
    try {
        const action = new WindowAction(str);
        const id = action.run();
        sendReply(id ? `OK ${id}` : "NotFound");
    } catch (err) {
        sendReply(`ERROR ${err}`);
    }
};

const fetchNextAction = () => dbus(
    "fetchNextAction",
    processAction
);

// --- Background service (uk.tvidal.server) integration -----------------------
// The systemd/D-Bus-activated service tells us which windows to watch
// (GetTargets) and what to do with each match (WindowAction). We pull the target
// list on load and whenever KWinToolReconfigure fires, match it against
// windowAdded locally, then ask the service for the actions to apply so the Rust
// side stays the single source of truth for the rules.

const SERVER_BUS = "uk.tvidal.server";
const SERVER_PATH = "/KWinTool";
const SERVER_INTERFACE = "uk.tvidal.server";
const SEP_TARGET = "\n";

const serverDbus = (methodName, ...args) => callDBus(
    SERVER_BUS,
    SERVER_PATH,
    SERVER_INTERFACE,
    methodName,
    ...args
);

// A search-only matcher for one target line, e.g. "class=mpv&&title=ipcam1".
class Target {
    constructor(str) {
        this.search = str.split(SEP_SEARCH)
            .filter(s => !!s)
            .map(searchMatch);
    }

    matches(win) {
        return win.windowType === WIN_NORMAL
            && this.search.length > 0
            && this.search.every(s => s.matches(win));
    }
}

let targets = [];

function applyActions(win, actions) {
    actions.split(SEP_ACTION)
        .filter(a => !!a)
        .map(windowAction)
        .forEach(a => a.execute(win));
}

// Called for every window: if it matches a configured target, ask the service
// what to do with it (passing "caption:class") and apply the reply.
function handleWindow(win) {
    try {
        if (!targets.some(t => t.matches(win))) {
            return;
        }
        const window = `${win.caption}:${win.resourceClass}`;
        serverDbus("WindowAction", window, (actions) => {
            if (actions) {
                logDebug(win, `WindowAction(${window}) => ${actions}`);
                applyActions(win, actions);
            }
        });
    } catch (err) {
        logError(err);
    }
}

// Re-fetch the target list from the service and re-apply to open windows.
function reconfigure() {
    serverDbus("GetTargets", (reply) => {
        try {
            targets = (reply || "").split(SEP_TARGET)
                .filter(s => !!s)
                .map(s => new Target(s));
            if (debugEnabled) {
                console.log(`KWinTool: ${targets.length} target(s) loaded`);
            }
            workspace.stackingOrder.forEach(handleWindow);
        } catch (err) {
            logError(err);
        }
    });
}

registerShortcut("KWinToolDebugToggle", "Toggles DebugEnabled", null,
    () => console.log(`KWinTool: debugEnabled = ${debugEnabled = !debugEnabled}`)
);

registerShortcut("KWinToolAction", "Triggers a KWinTool action", null, fetchNextAction);
registerShortcut("KWinToolReconfigure", "Reload KWinTool window rules", null, reconfigure);

// `workspace` is a KWin runtime global; it is absent under the Node test harness
// (which drives the exported functions directly), so guard the live wiring.
if (typeof workspace !== "undefined") {
    workspace.windowAdded.connect(handleWindow);
    reconfigure();
}

console.log("KWinTool: KWin Script Loaded");

// Test-only hook: `module` is undefined inside the KWin (QJSEngine) runtime, so
// this block is skipped there and has no effect on the loaded script. Under
// Node it exposes the parser/matcher to the test harness, which drives the same
// wire protocol the Rust side emits. See kwin/test/main.test.mjs.
if (typeof module !== "undefined" && module.exports) {
    module.exports = {
        WindowAction, processAction, searchMatch, windowAction, parse,
        Target, applyActions, handleWindow, reconfigure,
    };
}
