//! `m.*` MethodSpec table — **the single declaration source** (§3.1).
//!
//! Rows come from the O10 inventory (`agent-works/docs/p3-capability-inventory.md`):
//! the mu unified-UI verbs cut by **what actually lands on Android** —
//! the broker's `Main.java` dispatch table, not the Python surface. The six
//! desktop-mouse verbs (`move`/`button_down`/`button_up`/`wheel`/`key_down`/
//! `key_up`/`release_all`) land as `UNSUPPORTED_CAPABILITY` on Android and are
//! **not declared** here: declared = enforceable, and a seed line that cannot
//! land is a lie the model pays for every turn.
//!
//! One `MethodSpec` name is one `MethodPolicy` row (P2 lesson): act verbs ask,
//! observe verbs flow, and the two never share a name. Target dispatch (node
//! vs point) rides inside one name — same decision either way, so no split.
//!
//! The capture column (inventory §3.2) splits the same way: `m.observe` mints
//! one capture per look, and every verb that acts on a capture-derived target
//! spends one — one action attempt per look, reuse refused at the chokepoint.
//! Blind effects (keys, app launch, shell, files) draw no capture authority.
//!
//! File access splits at the decision boundary too: `m.file_read` flows,
//! `m.file_write` (write/mkdir/remove/move) asks. Device file reads are
//! `device_capture` — the phone's storage is device state, not the host's `FsRead`.

use phi_cap::{device_capture, device_input};
use phi_ns::{CaptureUse, MethodSpec, m};

/// The whole `m.*` face, in seed-list order.
pub fn method_table() -> Vec<MethodSpec> {
    vec![
        // ---- look ----
        m(
            "m.observe",
            &[device_capture(&["phone"])],
            "observe(mode=\"ax\"|\"vision\", app=None): read the screen as an accessibility tree (node handles with roles/text) or as a screenshot with visual regions",
            true,
        )
        .capture(CaptureUse::Mint),
        m(
            "m.apps",
            &[device_capture(&["phone"])],
            "apps(): list installed/launchable apps",
            false,
        ),
        m(
            "m.file_read",
            &[device_capture(&["phone"])],
            "file_read(path, op=\"read\"|\"stat\"|\"list\"): read a device file, stat it, or list its directory",
            false,
        ),
        // ---- act: every verb below asks (MethodPolicy) ----
        m(
            "m.click",
            &[device_input(&["phone"])],
            "click(target): tap a node from m.observe(mode=\"ax\") or an [x, y] point",
            false,
        )
        .capture(CaptureUse::Consume),
        m(
            "m.scroll",
            &[device_input(&["phone"])],
            "scroll(target, direction, amount=1.0): scroll a node or the viewport \"up\"/\"down\"/\"left\"/\"right\"",
            false,
        )
        .capture(CaptureUse::Consume),
        m(
            "m.swipe",
            &[device_input(&["phone"])],
            "swipe(start, end, duration_ms=500): swipe from start to end (points or nodes)",
            false,
        )
        .capture(CaptureUse::Consume),
        m(
            "m.long_press",
            &[device_input(&["phone"])],
            "long_press(target, duration_ms=800): long-press a node from m.observe(mode=\"ax\") or an [x, y] point",
            false,
        )
        .capture(CaptureUse::Consume),
        m(
            "m.drag",
            &[device_input(&["phone"])],
            "drag(start, end, duration_ms=500, hold_ms=None): drag from start to end",
            false,
        )
        .capture(CaptureUse::Consume),
        m(
            "m.set_text",
            &[device_input(&["phone"])],
            "set_text(target, text): fill a node's editable text",
            false,
        )
        .capture(CaptureUse::Consume),
        m(
            "m.select_text",
            &[device_input(&["phone"])],
            "select_text(target, start, end): select a character range inside a node",
            false,
        )
        .capture(CaptureUse::Consume),
        m(
            "m.perform",
            &[device_input(&["phone"])],
            "perform(target, action): run a node action — \"click\"/\"long_click\"/\"focus\"/\"select\"/\"expand\"/\"collapse\"/\"copy\"/\"paste\"/\"cut\"",
            false,
        )
        .capture(CaptureUse::Consume),
        m(
            "m.text",
            &[device_input(&["phone"])],
            "text(text, target=None): type text at the current focus (needs the Adb keyboard IME)",
            false,
        )
        .capture(CaptureUse::Consume),
        m(
            "m.press_key",
            &[device_input(&["phone"])],
            "press_key(key): press one key — \"enter\"/\"tab\"/\"esc\"/\"backspace\"/\"delete\"/\"space\"/\"up\"/\"down\"/\"left\"/\"right\"/\"home\"/\"end\"",
            false,
        ),
        m(
            "m.system",
            &[device_input(&["phone"])],
            "system(button): press \"back\"/\"home\"/\"recents\"",
            false,
        ),
        m(
            "m.launch",
            &[device_input(&["phone"])],
            "launch(app): launch an app by package name",
            false,
        ),
        m(
            "m.file_write",
            &[device_input(&["phone"])],
            "file_write(op, path, ...): change device files — op=\"write\"|\"mkdir\"|\"remove\"|\"move\"",
            false,
        ),
        m(
            "m.shell",
            &[device_input(&["phone"])],
            "shell(argv, timeout_ms=10000): run a command on the device, argv as a list",
            false,
        ),
        // ---- meta: waits and probes ----
        m(
            "m.stabilize",
            &[],
            "stabilize(timeout_ms=5000): wait until the screen stops changing",
            false,
        ),
        m(
            "m.describe",
            &[],
            "describe(): read the device's capability/security self-report (IME, privileges)",
            false,
        ),
        m(
            "m.status",
            &[],
            "status(action_id): query the device's outcome ledger — did a past action land, or is its result unknown?",
            false,
        ),
    ]
}
