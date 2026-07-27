// Cross-language contract tests for the KWin script.
//
// The Rust side (see src/cmd.rs) serialises every request into a wire string of
// the form `search && search && action;action`. These tests feed the KWin
// script the *exact* strings that cmd.rs emits and assert how it selects and
// acts on windows, so the two halves cannot drift apart silently.
//
// Run with:  node --test kwin/test/
//
// The script targets KWin's QJSEngine, which exposes globals such as
// `workspace`, `callDBus`, `registerShortcut`, `KWin` and `QTimer`. We install
// minimal stubs for those before loading it, then drive the exported
// `processAction(wire)` entry point directly and capture the reply it would
// have sent back over D-Bus.

import { test } from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const require = createRequire(import.meta.url);
const here = dirname(fileURLToPath(import.meta.url));

// --- KWin runtime stubs, installed before the script is loaded ---------------

let lastReply;
// Canned replies for the background-service methods (method -> reply string) and
// a log of the calls the script made, so tests can drive GetTargets/WindowAction.
let dbusResponses = {};
let dbusCalls = [];

globalThis.registerShortcut = () => {};
globalThis.callDBus = (_bus, _path, _iface, method, ...args) => {
    // The script replies to the one-shot client via callDBus(..., "sendReply", …).
    if (method === "sendReply") {
        lastReply = args[0];
        return;
    }
    // Other calls (GetTargets/WindowAction) may pass a trailing reply callback.
    const callback = typeof args[args.length - 1] === "function" ? args.pop() : null;
    dbusCalls.push({ method, args });
    if (callback && Object.prototype.hasOwnProperty.call(dbusResponses, method)) {
        callback(dbusResponses[method]);
    }
};
globalThis.KWin = { MaximizeArea: 0 };
// GeometryAction defers its work to a QTimer; run it synchronously in tests.
globalThis.QTimer = class {
    constructor() {
        this.timeout = { connect: (fn) => (this._fn = fn) };
    }
    start() {
        if (this._fn) this._fn();
    }
};

// Loading the script prints a one-off banner and registers shortcuts; silence
// the banner so it does not pollute the test output.
const banner = console.log;
console.log = () => {};
const { processAction, reconfigure, handleWindow, applyActions } = require(join(here, "..", "contents", "code", "main.js"));
console.log = banner;

// --- Fixtures ----------------------------------------------------------------

/** A fake KWin Window exposing only the surface the script touches. */
function makeWindow(props = {}) {
    return {
        windowType: 0, // WIN_NORMAL
        resourceName: "",
        resourceClass: "",
        caption: "",
        internalId: "{fake-id}",
        desktops: [],
        onAllDesktops: false,
        noBorder: false,
        keepBelow: false,
        frameGeometry: { x: 0, y: 0, width: 0, height: 0 },
        setMaximize(vertical, horizontal) {
            this._maximize = { vertical, horizontal };
        },
        ...props,
    };
}

/** Installs a fresh `workspace` and clears the captured reply. */
function scenario({ windows = [], active = null, screens = [], desktops = [] } = {}) {
    lastReply = undefined;
    dbusResponses = {};
    dbusCalls = [];
    globalThis.workspace = {
        stackingOrder: windows,
        activeWindow: active,
        screens,
        desktops,
        currentDesktop: desktops[0] ?? null,
        clientArea: () => ({ left: 0, top: 0, width: 1920, height: 1080 }),
        sendClientToScreen(win, screen) {
            win._screen = screen;
        },
    };
    return globalThis.workspace;
}

// --- Matching & the OK / NotFound / ERROR reply protocol ---------------------

test("focuses a window matched by the default name criterion", () => {
    const dolphin = makeWindow({ resourceName: "dolphin", internalId: "{d}" });
    const ws = scenario({ windows: [dolphin], active: null });

    processAction("name=^dolphin$&&activate"); // cmd.rs: `kwintool dolphin`

    assert.equal(lastReply, "OK {d}");
    assert.equal(ws.activeWindow, dolphin);
});

test("replies NotFound when nothing matches", () => {
    const firefox = makeWindow({ resourceName: "firefox" });
    const ws = scenario({ windows: [firefox], active: firefox });

    processAction("name=^dolphin$&&activate");

    assert.equal(lastReply, "NotFound");
    assert.equal(ws.activeWindow, firefox); // untouched
});

test("matches an explicit class criterion", () => {
    const konsole = makeWindow({ resourceClass: "konsole", internalId: "{k}" });
    scenario({ windows: [konsole] });

    processAction("class=konsole&&activate"); // cmd.rs: `-c konsole`

    assert.equal(lastReply, "OK {k}");
});

test("negated criterion excludes the matching window", () => {
    const fleet = makeWindow({ resourceClass: "fleet", internalId: "{f}" });
    const konsole = makeWindow({ resourceClass: "konsole", internalId: "{k}" });
    const ws = scenario({ windows: [fleet, konsole] });

    processAction("class!=fleet&&activate"); // cmd.rs: `--class '!fleet'`

    assert.equal(lastReply, "OK {k}");
    assert.equal(ws.activeWindow, konsole);
});

test("a lone negated-out window yields NotFound", () => {
    const fleet = makeWindow({ resourceClass: "fleet" });
    scenario({ windows: [fleet] });

    processAction("class!=fleet&&activate");

    assert.equal(lastReply, "NotFound");
});

test("matches a title substring, case-insensitively", () => {
    const meet = makeWindow({ caption: "Project sync - Google Meet", internalId: "{m}" });
    scenario({ windows: [meet] });

    processAction("title=Google Meet&&activate"); // cmd.rs: `-t 'Google Meet'`

    assert.equal(lastReply, "OK {m}");
});

// --- The focus-or-cycle behaviour the earlier regression fix locked in -------

test("cycles to the next match when the active window already matches", () => {
    const a = makeWindow({ resourceName: "alacritty", internalId: "{a}" });
    const b = makeWindow({ resourceName: "alacritty", internalId: "{b}" });
    const ws = scenario({ windows: [a, b], active: a });

    processAction("name=^alacritty$&&activate");

    assert.equal(ws.activeWindow, b); // moved to the other match
    assert.equal(lastReply, "OK {b}");
});

test("re-triggering on the only match (already active) still succeeds", () => {
    // Regression guard: this must reply OK, not NotFound — otherwise
    // focus-or-start would spawn a duplicate on every repeat invocation.
    const only = makeWindow({ resourceName: "alacritty", internalId: "{a}" });
    const ws = scenario({ windows: [only], active: only });

    processAction("name=^alacritty$&&activate");

    assert.equal(lastReply, "OK {a}");
    assert.equal(ws.activeWindow, only);
});

test("an unknown search field is reported as ERROR", () => {
    scenario({ windows: [makeWindow()] });

    processAction("bogus=x&&activate");

    assert.match(lastReply, /^ERROR /);
    assert.match(lastReply, /invalid search criteria/);
});

// --- Actions -----------------------------------------------------------------

test("moves the matched window to a virtual desktop, then activates it", () => {
    const d0 = { id: "d0" };
    const d1 = { id: "d1" };
    const win = makeWindow({ resourceName: "x", internalId: "{x}" });
    const ws = scenario({ windows: [win], desktops: [d0, d1] });

    processAction("name=^x$&&desktop=1;activate"); // cmd.rs: `-D 2` (1-based)

    assert.deepEqual(win.desktops, [d1]);
    assert.equal(ws.activeWindow, win);
    assert.equal(lastReply, "OK {x}");
});

test("sends the matched window to a screen matched by connector name", () => {
    // The regex must match the connector `name` (DP-2), not the EDID `model`.
    const hdmi = { name: "HDMI-A-1", model: "LG TV", manufacturer: "LG" };
    const dp = { name: "DP-2", model: "LG Monitor", manufacturer: "LG" };
    const win = makeWindow({ resourceName: "x", internalId: "{x}" });
    const ws = scenario({ windows: [win], screens: [hdmi, dp] });

    processAction("name=^x$&&screen=DP;activate"); // cmd.rs: `-S DP`

    assert.equal(win._screen, dp);
    assert.equal(ws.activeWindow, win);
    assert.equal(lastReply, "OK {x}");
});

test("applies a proportional geometry to the matched window", () => {
    const win = makeWindow({ resourceName: "x", internalId: "{x}" });
    scenario({ windows: [win] });

    processAction("name=^x$&&geometry=w60%x20%m;activate"); // cmd.rs: `-g w60%x20%m`

    // clientArea is 1920x1080 at (0,0): width 60% -> 1152, left 20% -> 384.
    assert.equal(win.frameGeometry.width, 1152);
    assert.equal(win.frameGeometry.x, 384);
    assert.equal(lastReply, "OK {x}");
});

test("sizes are proportional to the area, not offset by the screen origin", () => {
    const win = makeWindow({ resourceName: "x", internalId: "{x}" });
    scenario({ windows: [win] });
    // A second screen at x=1920: sizes must not pick up that 1920 offset.
    workspace.clientArea = () => ({ left: 1920, top: 0, width: 1920, height: 1080 });

    processAction("name=^x$&&geometry=x10%w50%;activate");

    assert.equal(win.frameGeometry.x, 1920 + 192); // origin + 10% of width
    assert.equal(win.frameGeometry.width, 960); // 50% of width, no origin added
});

test("vertical maximize sets the width and maximizes vertically only", () => {
    const win = makeWindow({ resourceName: "x", internalId: "{x}" });
    scenario({ windows: [win] });

    processAction("name=^x$&&geometry=x17%w66%v;activate");

    // The target width is applied (66% of 1920) and the window is maximized
    // vertically, not horizontally — the ultra-wide regression.
    assert.equal(Math.round(win.frameGeometry.width), 1267);
    assert.deepEqual(win._maximize, { vertical: true, horizontal: false });
});

test("searching by desktop index selects the window on that desktop", () => {
    // On the wire, `desktop=N` is the 0-based index into workspace.desktops,
    // the same numbering MoveToDesktopAction uses. cmd.rs converts the 1-based
    // CLI value into this wire index, so `-d 3` sends `desktop=2`.
    const d0 = { id: "d0" };
    const d1 = { id: "d1" };
    const d2 = { id: "d2" };
    const onD0 = makeWindow({ resourceName: "x", internalId: "{d0}", desktops: [d0] });
    const onD2 = makeWindow({ resourceName: "x", internalId: "{d2}", desktops: [d2] });
    scenario({ windows: [onD0, onD2], desktops: [d0, d1, d2] });

    processAction("desktop=2&&activate"); // cmd.rs: `-d 3` (1-based)

    assert.equal(lastReply, "OK {d2}"); // the window on desktop index 2
});

test("a window on all desktops does not match a specific desktop index", () => {
    const d0 = { id: "d0" };
    const allDesktops = makeWindow({ resourceName: "x", desktops: [] });
    scenario({ windows: [allDesktops], desktops: [d0] });

    processAction("desktop=0&&activate"); // cmd.rs: `-d 1` (1-based)

    assert.equal(lastReply, "NotFound");
});

// --- Title composite matching & the new toggle actions -----------------------

test("title matches against the caption:class composite", () => {
    // Empty-caption JetBrains window: the composite is ":jetbrains-fleet".
    const jb = makeWindow({ caption: "", resourceClass: "jetbrains-fleet", internalId: "{j}" });
    scenario({ windows: [jb] });

    processAction("title=^:jetbrains-&&activate"); // anchors on the class suffix

    assert.equal(lastReply, "OK {j}");
});

test("noborder action toggles the window frame", () => {
    const win = makeWindow({ resourceClass: "x" });
    scenario({ windows: [win] });

    applyActions(win, "noborder=true");

    assert.equal(win.noBorder, true);
});

test("maximize=false un-maximizes the window", () => {
    const win = makeWindow({ resourceClass: "org-diylc-DIYLC" });
    scenario({ windows: [win] });

    applyActions(win, "maximize=false");

    assert.deepEqual(win._maximize, { vertical: false, horizontal: false });
});

test("keepbelow action lowers the window in the stack", () => {
    const win = makeWindow({ resourceClass: "mpv" });
    scenario({ windows: [win] });

    applyActions(win, "keepbelow=true");

    assert.equal(win.keepBelow, true);
});

// --- Background service (windowAdded) integration ----------------------------

test("reconfigure applies the service's action to a matching open window", () => {
    const term = makeWindow({ resourceClass: "alacritty", caption: "Terminal", internalId: "{a}" });
    const ws = scenario({ windows: [term], desktops: [{ id: "d0" }, { id: "d1" }, { id: "d2" }] });
    // config::targets() serves the prefilter; config::action_for() the actions.
    dbusResponses["GetTargets"] = "class=alacritty";
    dbusResponses["WindowAction"] = "desktop=2"; // 0-based wire index

    reconfigure();

    // The script asks the service using the "caption:class" composite...
    const call = dbusCalls.find(c => c.method === "WindowAction");
    assert.deepEqual(call.args, ["Terminal:alacritty"]);
    // ...and applies the returned action to that window.
    assert.deepEqual(term.desktops, [ws.desktops[2]]);
});

test("a window matching no target is left untouched", () => {
    const editor = makeWindow({ resourceClass: "code", caption: "main.rs", internalId: "{c}" });
    scenario({ windows: [editor] });
    dbusResponses["GetTargets"] = "class=alacritty";

    reconfigure();

    assert.equal(dbusCalls.some((c) => c.method === "WindowAction"), false);
});

test("windowAdded consults the service only after targets are loaded", () => {
    const mpv = makeWindow({ resourceClass: "mpv", caption: "ipcam1", internalId: "{m}" });
    scenario({ windows: [], desktops: [{ id: "d0" }] });
    dbusResponses["GetTargets"] = "class=mpv&&title=ipcam1";
    dbusResponses["WindowAction"] = "geometry=x0y0";

    // Before reconfigure there are no targets, so a new window is ignored.
    handleWindow(mpv);
    assert.equal(dbusCalls.some((c) => c.method === "WindowAction"), false);

    // After loading targets, the same window is acted on.
    reconfigure();
    handleWindow(mpv);
    assert.equal(dbusCalls.some((c) => c.method === "WindowAction"), true);
});
