# kdetool

This project is aimed at allowing the user to manipulate aspects of KDE through shell scripts

The main motivation behind this project is to implement a "focus or execute feature" that became
really hard to implement with KDE 6 and Wayland.

I am also inspired by kdotool, I used it for a long time, but eventually the approach of
creating an ad-hoc script, registering, executing, unregistering and cleaning up became too
cumbersome and often left hanging files behind.

This implementation will add a single, long-lived KWin script that will be installed, registered
and started on service execution only after ensuring it is not already installed and running.

The service will also register itself as a DBus listener and expose a DBus interface to interact
with the KWin script. It should receive notifications of window created, window closed, change
in focused window, change in current desktop, window moved to another desktop and similar
events.

As this is supposed to be a simple command-line tool, we should keep external dependencies to a
minimum. We should try to use the most popular crates available for each specific task we need.

## Project Structure

```
+ kdetool
  |- src
  |  |- (rust source files)
  |- kwin
  |  |- contents
  |  |  |- code
  |  |  |  |- main.js       - kwin script to communicate with the tool
  |  |- metadata.json       - kwin script metadata  
```

Here are a few examples of dbus interfaces this program will need to use:

```
$ busctl --user --verbose introspect org.kde.KWin /KWin
NAME                                TYPE      SIGNATURE RESULT/VALUE FLAGS
org.freedesktop.DBus.Introspectable interface -         -            -
.Introspect                         method    -         s            -
org.freedesktop.DBus.Peer           interface -         -            -
.GetMachineId                       method    -         s            -
.Ping                               method    -         -            -
org.freedesktop.DBus.Properties     interface -         -            -
.Get                                method    ss        v            -
.GetAll                             method    s         a{sv}        -
.Set                                method    ssv       -            -
.PropertiesChanged                  signal    sa{sv}as  -            -
org.kde.KWin                        interface -         -            -
.activeOutputName                   method    -         s            -
.currentDesktop                     method    -         i            -
.getWindowInfo                      method    s         a{sv}        -
.killWindow                         method    -         -            no-reply
.nextDesktop                        method    -         -            -
.previousDesktop                    method    -         -            -
.queryWindowInfo                    method    -         a{sv}        -
.reconfigure                        method    -         -            no-reply
.replace                            method    -         -            -
.setCurrentDesktop                  method    i         b            -
.showDebugConsole                   method    -         -            -
.showDesktop                        method    b         -            no-reply
.supportInformation                 method    -         s            -
.showingDesktop                     property  b         false        emits-change
.reloadConfig                       signal    -         -            -
.showingDesktopChanged              signal    b         -            -

$ busctl --user --verbose introspect org.kde.KWin /component/kwin
NAME                                TYPE      SIGNATURE RESULT/VALUE  FLAGS
org.freedesktop.DBus.Introspectable interface -         -             -
.Introspect                         method    -         s             -
org.freedesktop.DBus.Peer           interface -         -             -
.GetMachineId                       method    -         s             -
.Ping                               method    -         -             -
org.freedesktop.DBus.Properties     interface -         -             -
.Get                                method    ss        v             -
.GetAll                             method    s         a{sv}         -
.Set                                method    ssv       -             -
.PropertiesChanged                  signal    sa{sv}as  -             -
org.kde.kglobalaccel.Component      interface -         -             -
.allShortcutInfos                   method    -         a(ssssssaiai) -
.allShortcutInfos                   method    s         a(ssssssaiai) -
.cleanUp                            method    -         b             -
.getShortcutContexts                method    -         as            -
.invokeShortcut                     method    s         -             -
.invokeShortcut                     method    ss        -             -
.isActive                           method    -         b             -
.shortcutNames                      method    -         as            -
.shortcutNames                      method    s         as            -
.friendlyName                       property  s         "KWin"        emits-change
.uniqueName                         property  s         "kwin"        emits-change
.globalShortcutPressed              signal    ssx       -             -
.globalShortcutReleased             signal    ssx       -             -
.globalShortcutRepeated             signal    ssx       -             -
```

The KWin Scripting API Reference can be found at:

- https://develop.kde.org/docs/plasma/kwin/api/

## External Dependencies

- Parse command line arguments
- Send and Receive DBus messages
- Write to the system journal
- Search for executable files in the path
- Search through the list of currently running processes

### Features

Many features will interact with a particular program, to do so, we must receive the program
executable file name and, if different, a regular expression to be matched against the resource
class of the program main window.

#### Inner workings

The registered kwin script can communicate with the tool by calling the methods on the DBus
interface. We will need to figure out better ways for the tool to send commands to the kwin
script, as KDE KWin DBus interface does not expose all the features we'll need.

One example is to get a list of currently open windows, with the title, classname, geometry,
etc... We could add a shortcut to trigger the script to send such list to the tool via DBus, but
the shortcut approach will only work for parameterless commands. We'll need to figure something
out to pass parameters to the script.

As the tool is mainly focused to be used in shell scripts, the output should always be easily
parseable with reduced useless information like titles, columns should be delimited by the tab
character unless explicitly requested otherwise.

Any informative message or error should be sent to the standard error, but inner debugging and
troubleshooting messages should be sent directly to the system logger via some library we will
search later.

### Usage:

```
kdetool
Arguments:
    <exe>       The name of the executable file
    [args...]  Command line arguments to be passed to the executable

Options:
    -c, --class <regex>             Regular expression to be matched against the resource class
                                    of the program main window
    -t, --title <regex>             Regular expression to be matched against the window title
    -d, --desktop <index>           Index of the desktop where the window will be searched
    -s, --screen <regex>            Search windows within the screens matching the regular expression    

    -D, --to-desktop <index>        Moves the window to the specified desktop
    -S, --to-screen <regex>         Moves the matched window to the first screen whose name 
                                    matches the provided regular expression
    -g, --geometry <geometry>       Sets the window position, size and maximize state 
                                    according to the <geometry> argument
                                    
    --list-desktops                 Prints information about all available desktops
    --list-screens                  Prints information about all available screens  
    -v, --verbose                   Enable verbose output
    -V, --version                   Print version information
    -h, --help                      Print help information
    
    When no search criteria is provided the executable name will be used to search the window 
resource class name.

    If the current active window matches the search criteria, the next window (if it exists),
 matching the search criteria will be focused instead.

Geometry:
    wNNN    width    
    hNNN    height
    xNNN    horizontal start position
    yNNN    vertical position
    m<V/H>  Maximized (Vertical with MV, Horizontal with MH or both with MVH or MHV)

        Any number can be terminated with a percent sign, indicating that number is proportional 
    to the aria available.

        A combination of values is permitted, for example:
        w60%x20%mV    Width 60%, Left = 20%, Vertically Maximized
        mVH           Maximized
        w1280h720x0y0 A 1280x720 pixels window on the top left corner

    The order of geometry parameters is irrelevant and the same parameter is defined twice, 
    the last will be used.
```

For example, if I run the tool as:

```shell
kdetool dolphin
```

1. We search the window stack list looking for a window whose classname matches the regular
   expression `/^dolphin$/`

2. If the currently active window matches the search criteria, we switch the focus to the next
   window that matches the criteria, if any

3. We execute actions to move the window to the specified screen or desktop on all windows that
   match the search criteria (we may change this behaviour later)

4. If a geometry was provided we compute the geometry according to the available area on the
   window output and resize the window

5. We maximize the window vertically, horizontally or both, according to the parameter received

6. If no window matches the search criteria, we check the list of running processes for the user
   for an executable with the same name

   1. If there is a program already running from the same file, it did not create any window,
      we print a warning on the standard error and exit with code 127
   2. If there is no program running, we start a new process on the parent namespace so it
      won't become a zombie process when this tool exits

7. If the executable is not found on the path, the tool returns with exit code 1

8. If no executable file is provided as command line argument, nothing will happen after the 
   commands executed via DBus and KWin Scripts are complete.

## Coding Standards

We will aim to follow TDD by having tests for the feature we are about to implement, before 
we actually implement it.

We will follow all rust standard coding practices and style.

We will divide the project into meaningful parts, to be defined later.