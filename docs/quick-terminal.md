# Quick terminal global shortcut

`volt-ui/src/quick_terminal.rs` supplies `GlobalHotKey::register(proxy) ->
anyhow::Result<GlobalHotKey>` on macOS. The default is Command+grave, using
physical ANSI grave keycode **50** and Carbon `cmdKey` (**256**), independent
of keyboard layout. No event taps or Accessibility permission are used.

## Opt in

Add to `config.toml`, then reload settings or restart Volt:

```toml
[quick_terminal]
enabled = true
```

Omitted or false means no global key registration and normal window lifecycle
is unchanged. This setting is macOS-only in effect. No settings UI is added.

## Host integration

App owns the guard, `ToggleQuickTerminal` event, and transient window lifecycle.
The window is created lazily on the first shortcut, starts hidden during setup,
then is shown, unminimized and focused after activating Volt. It is a floating
1000 by 500 logical-pixel terminal. Subsequent shortcuts toggle visibility;
traffic-light close and the native Close Window action hide it without killing
the shell. Closing the final terminal tab or shell exit destroys it; the next
shortcut recreates it.

Quick windows use custom tabs independently of the normal native-tab preference.
Only their NSWindow has tabbing disabled and AppKit restoration disabled;
normal window attributes and process automatic-tabbing preferences are unchanged.
The quick window is excluded before the session's window-count limit is applied.
If no normal window remains, saving does not overwrite the prior saved layout.

When the shortcut is successfully registered, Volt remains running after the last
normal window is closed, so the shortcut can reopen the quick terminal. Use Quit
to exit. Disabling the feature on settings reload unregisters the shortcut but
**preserves all PTYs and running jobs**. Any existing quick window is promoted
in place to a visible, unminimized normal-level standalone window with custom
tabs, ordinary close behavior, and session-persistence eligibility. It is not
removed from App and no replacement shell is spawned. Re-enabling the feature
starts with no owned quick window; a subsequent shortcut creates a new one. Failed registration leaves normal windows usable and displays
the failure in the existing top alert as well as stderr. Reloading retries a
failed registration; ordinary event-loop iterations do not retry it.

```text
main thread: register(proxy)
    |
    +-- Box<EventLoopProxy<VoltEvent>> (stable userdata address)
    +-- InstallEventHandler(application target, pressed events only)
    +-- RegisterEventHotKey(Command + keycode 50, exclusive)
            |
            +-- callback validates class, kind, signature, id, type, size
                    |
                    +-- enqueue ToggleQuickTerminal
                            |
                            +-- parent App handles window policy

main thread: exit / disable / drop guard (also used for partial-registration rollback)
    +-- UnregisterEventHotKey
    +-- RemoveEventHandler
    +-- drop proxy Box
```

Explicit hide calls the workers' existing clipboard-flight invalidation and
AX-cache invalidation hooks before hiding the NSWindow, even when it was not
focused. A stale async clipboard result is therefore refused without waiting
for Focused(false). AX render publication also observes the generic
focus/visibility gate, so background redraw cannot republish hidden output.

The guard is neither Send nor Sync. Registration checks `pthread_main_np`;
Drop asserts the main-thread invariant. Drop it before the event loop is
destroyed. Carbon teardown errors are logged. If handler removal fails, the
proxy is deliberately leaked rather than freeing still-referenced userdata.
An unregister failure can leave a registration until process exit; teardown
still attempts to remove the handler. A closed event loop does not panic.

Exclusive registration is required because Carbon's non-exclusive mode permits
multiple apps to register the same combination silently. A conflict returns
an error, including OSStatus; `eventHotKeyExistsErr` is -9878. Command+grave is
also commonly a macOS window-switching shortcut: actual system shortcut routing
and Secure Event Input behavior need manual validation. Do not promise that
global delivery overrides every system setting or secure-input state.

## ABI and validation

Declarations/constants were checked against the installed Xcode macOS SDK:

- HIToolbox `Events.h`: `kVK_ANSI_Grave = 0x32`, `cmdKey = 1 << 8`.
- `CarbonEvents.h`: `EventHotKeyID`, `RegisterEventHotKey`,
  `typeEventHotKeyID = 'hkid'`, pressed kind 5, exclusive option 1.
- `CarbonEventsCore.h`: `EventTypeSpec`, callback returning signed OSStatus,
  `InstallEventHandler`, `GetEventParameter`, keyboard class `'keyb'`,
  direct-object parameter `'----'`, error codes.
- SDK `MacTypes.h`: OSStatus is signed 32-bit; ItemCount and ByteCount are
  **unsigned long** (`c_ulong`), including GetEventParameter's size output.

Unit tests cover constants, event filtering (including unrelated IDs and key
release), and C struct layout. They never register a global shortcut.

Validation in the public repo: `cargo check -p volt-ui` and all six
quick-terminal-related tests passed. These include in-place promotion/no-drop,
session/lifetime policy, ABI/filter/constants, and the clipboard worker's
quick-hide-before-focus-event regression. No test registered a global key.
The full UI run reported 223 passed, 1 failed, 3 ignored: the unrelated clipboard
timeout/reap test failed with a missing temporary file, then passed on an
isolated rerun. Native shortcut/window behavior and live-job continuity still
need manual validation.

Manually verify background-app activation, repeated
show/hide, a conflict with an existing registration, drop/re-register, shutdown,
and keyboard-layout behavior. Build/test success alone is not native shortcut
delivery evidence.
