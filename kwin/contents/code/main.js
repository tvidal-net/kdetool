const BUS_NAME = "uk.tvidal";
const OBJECT_PATH = "/KWinTool";
const INTERFACE = "uk.tvidal.KWinTool";

const WIN_NORMAL = 0;
const REGEX_FLAGS = "i";
const CH_NOT = "!";

const SEP_SEARCH = "&&";
const SEP_ACTION = ";";
const SEP_VALUE = "=";

const RE_NAME = /^[a-z]+/i;
const RE_GEO = /([xywh])([0-9%]+)/gi

const DELAY_MS = 50;

let debugEnabled = true;

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
            return new SimpleMatch(search, w => w.desktops ? w.desktop[0].x11DesktopNumber : -1);
        case "name":
            return new RegExpMatch(search, w => w.resourceName);
        case "class":
            return new RegExpMatch(search, w => w.resourceClass);
        case "title":
            return new RegExpMatch(search, w => w.caption);
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
            win.desktops = [workspace.desktops[this.desktop]];
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
        const screen = workspace.screens
            .find(s => this.screen.test(s.model));

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

function computeGeometry(area, value) {
    const n = parseFloat(value);
    return /%/.test(value) ? n * area / 100.0 : n;
}

class GeometryAction {

    constructor(geometry) {
        this.geometry = {};
        let match = [];
        while (match = RE_GEO.exec(geometry)) {
            this.geometry[match[1].toUpperCase()] = match[2];
        }
    }

    execute(win) {
        const target = this.geometry;
        const timer = new QTimer();
        timer.singleShot = true;
        timer.interval = DELAY_MS;
        timer.timeout.connect(() => {
            const a = workspace.clientArea(KWin.MaximizeArea, win);
            let maxH = false, maxV = false;
            let geo = {};
            for (let ch in target) {
                const value = target[ch];
                switch (ch) {
                    case "V":
                        maxV = true;
                        break;

                    case "M":
                        maxV = maxH = true;
                        break

                    case "W":
                        ch = "width";
                    case "X":
                        geo[ch] = computeGeometry(a.width, value);
                        break;

                    case "H":
                        ch = "height";
                    case "Y":
                        geo[ch] = computeGeometry(a.height, value);
                        break;
                }
            }
            win.frameGeometry = Object.assign({}, win.frameGeometry, geo);
            win.setMaximize(maxV, maxH);
            logDebug(win, `Geometry: ${this} => ${JSON.stringify(geo)}`);
        });
        timer.start();
    }

    toString() {
        return `Geometry ${JSON.stringify(this.geometry)}`;
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
            && workspace.activeWindow !== win
            && this.search.every(s => s.matches(win));
    }

    run() {
        const win = workspace.stackingOrder
            .find(w => this.matches(w));

        if (win) {
            logDebug(win, `Matches ${this}`);
            this.actions.forEach(a => a.execute(win));
            return win.internalId;
        }
        return null;
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
        sendReply(`ERROR: ${err}`);
    }
};

const fetchNextAction = () => dbus(
    "fetchNextAction",
    processAction
);

registerShortcut("KWinToolDebugToggle", "Toggles DebugEnabled", null,
    () => console.log(`KWinTool: debugEnabled = ${debugEnabled = !debugEnabled}`)
);

registerShortcut("KWinToolAction", "Triggers a KWinTool action", null, fetchNextAction);
console.log("KWinTool: KWin Script Loaded");
