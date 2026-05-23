// kdetool KWin script.
//
// A KWin script cannot register a DBus object or subscribe to DBus signals:
// the scripting sandbox only exposes callDBus(), a one-shot outbound method
// call. We therefore subscribe to KWin's own `workspace` signals (which do
// fire repeatedly, once per event) and forward each event to the kdetool
// service. The only thing we can *receive* over DBus is the reply to our own
// call, so we print its first argument to kDebug (visible via journalctl).
const SERVICE = "uk.tvidal.KDETool";
const PATH = "/Windows";
const INTERFACE = "uk.tvidal.WindowManager";

function windowId(window) {
  return window ? String(window.internalId) : "";
}

// Calls INTERFACE.<method> on the kdetool service and prints the first value
// of the reply, which is the only data a KWin script can receive over DBus.
function report(method, window) {
  callDBus(
    SERVICE,
    PATH,
    INTERFACE,
    method,
    windowId(window),
    window ? window.resourceClass : "",
    window ? window.caption : "",
    function (received) {
      print("kdetool:", method, "received:", received);
    }
  );
}

workspace.windowActivated.connect(function (window) {
  report("windowActivated", window);
});
workspace.windowAdded.connect(function (window) {
  report("windowAdded", window);
});
workspace.windowRemoved.connect(function (window) {
  report("windowRemoved", window);
});
workspace.currentDesktopChanged.connect(function () {
  report("currentDesktopChanged", workspace.activeWindow);
});

print("kdetool: KWin script loaded, forwarding events to", SERVICE);
