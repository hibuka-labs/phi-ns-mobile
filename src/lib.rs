//! # phi-ns-mobile — the `m.*` capability pack
//!
//! The phone face over the mu broker (O10 inventory): observe the screen as
//! an AX tree or a screenshot, act on nodes and points, touch the device's
//! files and shell. What the model writes is code against `m.*`:
//!
//! ```python
//! obs = m.observe("ax")
//! for n in obs.nodes:
//!     if n.text == "Settings":
//!         n.click()
//!         break
//! shot = m.observe("vision")           # -> Obs with .image (ImageHandle)
//! ```
//!
//! The pack speaks only the `phi-ns` contract over [`Transport`]; assembly
//! (`.with_ns`) and policy live above it (§9.3: `phi-ns-* ↛ codexec`). The
//! production transport is [`phi_ns::remote::ProcessTransport`] wrapping
//! `phi-wire-client`; the stub is the mu broker itself, so this pack never writes
//! transport code — only the method → wire mapping and the target geometry.
//!
//! Wire facts this pack is built on (evidence: `adapters/android/agent/
//! Main.java` + `adapters/android/host/src/main.rs` + `python/mai_ui/`):
//!
//! * `op`/`_request_id`/flattened params in, `result`|`error` out (phi-wire-protocol)
//! * node actions carry `node_id` + `revision` (+ `target_token`) — stale
//!   revisions are refused (`STALE_TARGET`), never silently re-targeted
//! * point actions carry `x`,`y` and may assert `frame_width/height/rotation`
//! * the six desktop-mouse verbs do not land on Android and are not declared
//!   (inventory §2.1)

pub mod methods;

use async_trait::async_trait;
use phi_ns::{
    CAPTURE_KEY, HostCall, MethodSpec, NamespaceProvider, NsError, NsObject, WorkerSink,
    remote::Transport,
};
use serde_json::{Map, Value, json};
use std::sync::{Arc, Mutex};

const PY_SHIM: &str = r#"
class ImageHandle:
    """A picture the code can pass around and the model can look at."""
    def __init__(self, d):
        self.path = d.get("path")
        self.mime = d.get("mime", "image/png")
        self.alt = d.get("alt", "")

    def save(self, dst):
        import shutil
        shutil.copyfile(self.path, dst)
        return dst

    def __repr__(self):
        return "ImageHandle(%r)" % self.alt

    def _phi_summary_(self):
        return self.alt


class Node:
    """One UI node handle from m.observe(mode="ax"): reads plus the actions."""
    def __init__(self, d, revision):
        self.id = d.get("id")
        self.role = d.get("role", "")
        self.name = d.get("name", "")
        self.text = d.get("text", "")
        self.bounds = d.get("bounds")
        self._wire = {"node_id": self.id, "revision": revision,
                      "target_token": d.get("target_token", "")}

    def click(self):
        return _phi_call("m.click", [self._wire])

    def set_text(self, text):
        return _phi_call("m.set_text", [self._wire, text])

    def select_text(self, start, end):
        return _phi_call("m.select_text", [self._wire, start, end])

    def perform(self, action):
        return _phi_call("m.perform", [self._wire, action])

    def scroll(self, direction, amount=1.0):
        return _phi_call("m.scroll", [self._wire, direction, amount])

    def long_press(self, duration_ms=800):
        return _phi_call("m.long_press", [self._wire, duration_ms])

    def __repr__(self):
        return "Node(%s, %r)" % (self.role, self.text)

    def _phi_summary_(self):
        return "%s %r" % (self.role, self.text)


class Obs:
    """What m.observe saw: nodes (ax), or image + regions (vision)."""
    def __init__(self, d):
        self.revision = d.get("revision", "")
        self.width = d.get("width", 0)
        self.height = d.get("height", 0)
        self.nodes = [Node(n, self.revision) for n in d.get("nodes", [])]
        self.regions = d.get("regions", [])
        imgs = d.get("images", [])
        self.image = ImageHandle(imgs[0]) if imgs else None
        # The one-use capture id the host minted for this look — every action
        # spends one, and a second act off this look is refused.
        self.capture_id = d.get("capture_id", "")
        self.capture = d.get("capture", {})

    def _phi_summary_(self):
        return "rev=%s %dx%d, %d nodes" % (self.revision, self.width, self.height, len(self.nodes))


def _wire_of(target):
    return target._wire if isinstance(target, Node) else target


class MobileNs:
    def observe(self, mode="ax", app=None):
        return Obs(_phi_call("m.observe", [mode, app]))

    def apps(self):
        return _phi_call("m.apps", [])

    def file_read(self, path, op="read"):
        return _phi_call("m.file_read", [path, op])

    def file_write(self, op, path, **fields):
        return _phi_call("m.file_write", [op, path, fields])

    def click(self, target):
        return _phi_call("m.click", [_wire_of(target)])

    def scroll(self, target, direction, amount=1.0):
        return _phi_call("m.scroll", [_wire_of(target), direction, amount])

    def swipe(self, start, end, duration_ms=500):
        return _phi_call("m.swipe", [start, end, duration_ms])

    def long_press(self, target, duration_ms=800):
        return _phi_call("m.long_press", [_wire_of(target), duration_ms])

    def drag(self, start, end, duration_ms=500, hold_ms=None):
        return _phi_call("m.drag", [start, end, duration_ms, hold_ms])

    def set_text(self, target, text):
        return _phi_call("m.set_text", [_wire_of(target), text])

    def select_text(self, target, start, end):
        return _phi_call("m.select_text", [_wire_of(target), start, end])

    def perform(self, target, action):
        return _phi_call("m.perform", [_wire_of(target), action])

    def text(self, text, target=None):
        return _phi_call("m.text", [text, _wire_of(target) if target is not None else None])

    def press_key(self, key):
        return _phi_call("m.press_key", [key])

    def system(self, button):
        return _phi_call("m.system", [button])

    def launch(self, app):
        return _phi_call("m.launch", [app])

    def shell(self, argv, timeout_ms=10000):
        return _phi_call("m.shell", [argv, timeout_ms])

    def stabilize(self, timeout_ms=5000):
        return _phi_call("m.stabilize", [timeout_ms])

    def describe(self):
        return _phi_call("m.describe", [])

    def status(self, action_id):
        return _phi_call("m.status", [action_id])


m = MobileNs()
"#;

/// Geometry of the most recent capture — the fallback frame for point
/// targets and the `frame_*` assertion the wire checks against live display
/// metadata (inventory §3.2: a coordinate without its capture is a weapon).
#[derive(Debug, Default, Clone)]
struct Frame {
    width: i64,
    height: i64,
    rotation: i64,
}

/// The target a method acts on, after the shim's normalization.
#[derive(Debug, Clone)]
enum Target {
    Node {
        node_id: String,
        revision: String,
        target_token: String,
    },
    Point {
        x: f64,
        y: f64,
    },
}

impl Target {
    fn parse(value: &Value) -> Result<Self, NsError> {
        if let Some(obj) = value.as_object() {
            if let Some(id) = obj.get("node_id").and_then(Value::as_str) {
                return Ok(Self::Node {
                    node_id: id.to_string(),
                    revision: obj
                        .get("revision")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    target_token: obj
                        .get("target_token")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                });
            }
            let (x, y) = point_fields(obj)?;
            return Ok(Self::Point { x, y });
        }
        if let Some(pair) = value.as_array()
            && pair.len() == 2
        {
            let x = pair[0].as_f64().ok_or(point_err())?;
            let y = pair[1].as_f64().ok_or(point_err())?;
            return Ok(Self::Point { x, y });
        }
        Err(NsError::Contract(format!(
            "target must be a node handle or [x, y], got {value}"
        )))
    }

    /// Wire fields for the node/point branches the executor dispatches on.
    fn wire_fields(&self, frame: &Frame) -> Map<String, Value> {
        let mut out = Map::new();
        match self {
            Self::Node {
                node_id,
                revision,
                target_token,
            } => {
                out.insert("node_id".into(), json!(node_id));
                out.insert("revision".into(), json!(revision));
                if !target_token.is_empty() {
                    out.insert("target_token".into(), json!(target_token));
                }
            }
            Self::Point { x, y } => {
                out.insert("x".into(), json!(x));
                out.insert("y".into(), json!(y));
                // Assert the capture's geometry so a stale click on a moved
                // display is refused, not applied (Main.java STALE_TARGET).
                if frame.width > 0 {
                    out.insert("frame_width".into(), json!(frame.width));
                    out.insert("frame_height".into(), json!(frame.height));
                    out.insert("frame_rotation".into(), json!(frame.rotation));
                }
            }
        }
        out
    }
}

fn point_err() -> NsError {
    NsError::Contract("point target needs numeric x and y".into())
}

fn point_fields(obj: &Map<String, Value>) -> Result<(f64, f64), NsError> {
    let x = obj.get("x").and_then(Value::as_f64).ok_or_else(point_err)?;
    let y = obj.get("y").and_then(Value::as_f64).ok_or_else(point_err)?;
    Ok((x, y))
}

/// Named node actions → `AccessibilityNodeInfo` constants (the wire takes
/// the integer; the model writes the name).
fn node_action(name: &str) -> Result<i64, NsError> {
    Ok(match name {
        "click" => 16,
        "long_click" => 32,
        "focus" => 1,
        "clear_focus" => 2,
        "select" => 4,
        "clear_selection" => 8,
        "copy" => 16384,
        "paste" => 32768,
        "cut" => 65536,
        "scroll_forward" => 4096,
        "scroll_backward" => 8192,
        "expand" => 262144,
        "collapse" => 524288,
        other => {
            return Err(NsError::Contract(format!(
                "unknown node action {other:?} (click/long_click/focus/select/expand/collapse/copy/paste/cut)"
            )));
        }
    })
}

/// One declared method's request, ready for the wire.
enum Plan {
    Request {
        op: String,
        params: Map<String, Value>,
    },
    /// `m.scroll` on a point: the swipe trajectory is computed from the
    /// capture geometry exactly the way mu's own client computes it
    /// (`Vision.scroll`), then sent as one gesture.
    ScrollPoint {
        x: f64,
        y: f64,
        direction: String,
        amount: f64,
    },
}

impl Plan {
    fn request(op: &str, params: Map<String, Value>) -> Self {
        Self::Request {
            op: op.to_string(),
            params,
        }
    }
}

fn arg(args: &[Value], i: usize) -> Result<&Value, NsError> {
    args.get(i)
        .ok_or_else(|| NsError::Contract(format!("missing positional argument {i}")))
}

fn arg_str(args: &[Value], i: usize) -> Result<String, NsError> {
    Ok(arg(args, i)?
        .as_str()
        .ok_or_else(|| NsError::Contract(format!("argument {i} must be a string")))?
        .to_string())
}

fn arg_f64(args: &[Value], i: usize) -> Result<f64, NsError> {
    arg(args, i)?
        .as_f64()
        .ok_or_else(|| NsError::Contract(format!("argument {i} must be a number")))
}

fn action_params(
    target: &Value,
    frame: &Frame,
    extra: Map<String, Value>,
) -> Result<Map<String, Value>, NsError> {
    let mut params = Target::parse(target)?.wire_fields(frame);
    for (k, v) in extra {
        params.insert(k, v);
    }
    Ok(params)
}

/// Map one declared method + its positional args onto the mu wire.
fn plan(method: &str, args: &[Value], frame: &Frame) -> Result<Plan, NsError> {
    match method {
        "m.observe" => {
            let mode = arg_str(args, 0)?;
            let tree = match mode.as_str() {
                "ax" => true,
                "vision" => false,
                other => {
                    return Err(NsError::Contract(format!(
                        "observe mode must be \"ax\" or \"vision\", got {other:?}"
                    )));
                }
            };
            let mut params = Map::new();
            params.insert("tree".into(), json!(tree));
            params.insert("screenshot".into(), json!(!tree));
            if let Some(app) = arg(args, 1)?.as_str() {
                params.insert("app_id".into(), json!(app));
            }
            Ok(Plan::request("observe", params))
        }
        "m.apps" => Ok(Plan::request("list_apps", Map::new())),
        "m.file_read" => {
            let op = arg_str(args, 1)?;
            if !matches!(op.as_str(), "read" | "stat" | "list") {
                return Err(NsError::Contract(format!(
                    "file_read op must be read/stat/list, got {op:?}"
                )));
            }
            let mut params = Map::new();
            params.insert("operation".into(), json!(op));
            params.insert("path".into(), json!(arg_str(args, 0)?));
            Ok(Plan::request("file", params))
        }
        "m.file_write" => {
            let op = arg_str(args, 0)?;
            if !matches!(op.as_str(), "write" | "mkdir" | "remove" | "move") {
                return Err(NsError::Contract(format!(
                    "file_write op must be write/mkdir/remove/move, got {op:?}"
                )));
            }
            let mut params = Map::new();
            params.insert("operation".into(), json!(op));
            params.insert("path".into(), json!(arg_str(args, 1)?));
            if let Some(fields) = arg(args, 2)?.as_object() {
                for (k, v) in fields {
                    // Text content crosses as base64 on the wire (the
                    // executor's `data` field is base64 by contract).
                    if k == "data"
                        && let Some(text) = v.as_str()
                    {
                        params.insert(k.clone(), json!(base64_encode(text.as_bytes())));
                        continue;
                    }
                    params.insert(k.clone(), v.clone());
                }
            }
            Ok(Plan::request("file", params))
        }
        "m.click" => {
            let target = Target::parse(arg(args, 0)?)?;
            let (action, mut params) = match target {
                Target::Node { .. } => ("click", target.wire_fields(frame)),
                Target::Point { .. } => ("point_click", target.wire_fields(frame)),
            };
            params.insert("action".into(), json!(action));
            Ok(Plan::request("action", params))
        }
        "m.scroll" => {
            let target = Target::parse(arg(args, 0)?)?;
            let direction = arg_str(args, 1)?;
            if !matches!(direction.as_str(), "up" | "down" | "left" | "right") {
                return Err(NsError::Contract(format!(
                    "direction must be up/down/left/right, got {direction:?}"
                )));
            }
            let amount = arg_f64(args, 2)?;
            match target {
                Target::Node { .. } => {
                    let mut params = target.wire_fields(frame);
                    params.insert("action".into(), json!("node_scroll"));
                    params.insert("direction".into(), json!(direction));
                    params.insert("amount".into(), json!(amount));
                    params.insert("duration_ms".into(), json!(500));
                    Ok(Plan::request("action", params))
                }
                Target::Point { x, y } => Ok(Plan::ScrollPoint {
                    x,
                    y,
                    direction,
                    amount,
                }),
            }
        }
        "m.swipe" => {
            let start = Target::parse(arg(args, 0)?)?;
            let end = Target::parse(arg(args, 1)?)?;
            let (ex, ey) = match &end {
                Target::Point { x, y } => (*x, *y),
                _ => {
                    return Err(NsError::Contract(
                        "m.swipe takes two [x, y] points (or pt() handles)".into(),
                    ));
                }
            };
            let mut params = start.wire_fields(frame);
            params.insert("action".into(), json!("point_scroll"));
            params.insert("end_x".into(), json!(ex));
            params.insert("end_y".into(), json!(ey));
            params.insert("duration_ms".into(), json!(arg_f64(args, 2)? as i64));
            Ok(Plan::request("action", params))
        }
        "m.long_press" => {
            let target = Target::parse(arg(args, 0)?)?;
            let duration_ms = arg_f64(args, 1)? as i64;
            let (action, mut params) = match target {
                // A node long-press is the platform's ACTION_LONG_CLICK.
                Target::Node { .. } => ("perform", target.wire_fields(frame)),
                Target::Point { .. } => ("point_long_press", target.wire_fields(frame)),
            };
            params.insert("action".into(), json!(action));
            if action == "perform" {
                params.insert("node_action".into(), json!(32));
            } else {
                params.insert("duration_ms".into(), json!(duration_ms));
            }
            Ok(Plan::request("action", params))
        }
        "m.drag" => {
            let start = Target::parse(arg(args, 0)?)?;
            let end = Target::parse(arg(args, 1)?)?;
            let (ex, ey) = match &end {
                Target::Point { x, y } => (*x, *y),
                _ => {
                    return Err(NsError::Contract(
                        "m.drag takes two [x, y] points (or pt() handles)".into(),
                    ));
                }
            };
            let mut params = start.wire_fields(frame);
            params.insert("action".into(), json!("drag"));
            params.insert("end_x".into(), json!(ex));
            params.insert("end_y".into(), json!(ey));
            params.insert("duration_ms".into(), json!(arg_f64(args, 2)? as i64));
            if let Some(hold) = arg(args, 3)?.as_f64() {
                params.insert("hold_ms".into(), json!(hold as i64));
            }
            Ok(Plan::request("action", params))
        }
        "m.set_text" => {
            let mut params = action_params(arg(args, 0)?, frame, Map::new())?;
            if !params.contains_key("node_id") {
                return Err(NsError::Contract(
                    "m.set_text needs a node handle from m.observe(mode=\"ax\")".into(),
                ));
            }
            params.insert("action".into(), json!("set_text"));
            params.insert("text".into(), json!(arg_str(args, 1)?));
            Ok(Plan::request("action", params))
        }
        "m.select_text" => {
            let mut params = action_params(arg(args, 0)?, frame, Map::new())?;
            if !params.contains_key("node_id") {
                return Err(NsError::Contract(
                    "m.select_text needs a node handle from m.observe(mode=\"ax\")".into(),
                ));
            }
            params.insert("action".into(), json!("select_text"));
            params.insert("start".into(), json!(arg_f64(args, 1)? as i64));
            params.insert("end".into(), json!(arg_f64(args, 2)? as i64));
            Ok(Plan::request("action", params))
        }
        "m.perform" => {
            let mut params = action_params(arg(args, 0)?, frame, Map::new())?;
            if !params.contains_key("node_id") {
                return Err(NsError::Contract(
                    "m.perform needs a node handle from m.observe(mode=\"ax\")".into(),
                ));
            }
            params.insert("action".into(), json!("perform"));
            params.insert(
                "node_action".into(),
                json!(node_action(&arg_str(args, 1)?)?),
            );
            Ok(Plan::request("action", params))
        }
        "m.text" => {
            let mut params = Map::new();
            params.insert("action".into(), json!("input_text"));
            params.insert("text".into(), json!(arg_str(args, 0)?));
            let target = arg(args, 1)?;
            if !target.is_null() {
                for (k, v) in Target::parse(target)?.wire_fields(frame) {
                    params.insert(k, v);
                }
            }
            Ok(Plan::request("action", params))
        }
        "m.press_key" => {
            let mut params = Map::new();
            params.insert("action".into(), json!("press_key"));
            // The broker translates portable key names to Android key codes
            // at the provider boundary (`translate_portable_input`).
            params.insert("key".into(), json!(arg_str(args, 0)?));
            Ok(Plan::request("action", params))
        }
        "m.system" => {
            let button = arg_str(args, 0)?;
            if !matches!(button.as_str(), "back" | "home" | "recents") {
                return Err(NsError::Contract(format!(
                    "system button must be back/home/recents, got {button:?}"
                )));
            }
            let mut params = Map::new();
            params.insert("action".into(), json!("system"));
            params.insert("button".into(), json!(button));
            Ok(Plan::request("action", params))
        }
        "m.launch" => {
            let mut params = Map::new();
            params.insert("action".into(), json!("launch"));
            // The executor strips the "app:" marker from `app_id`.
            params.insert("app_id".into(), json!(format!("app:{}", arg_str(args, 0)?)));
            Ok(Plan::request("action", params))
        }
        "m.shell" => {
            let argv = arg(args, 0)?
                .as_array()
                .ok_or_else(|| NsError::Contract("shell argv must be a list".into()))?;
            let mut params = Map::new();
            params.insert("argv".into(), Value::Array(argv.clone()));
            params.insert("timeout_ms".into(), json!(arg_f64(args, 1)? as i64));
            Ok(Plan::request("shell", params))
        }
        "m.stabilize" => {
            let mut params = Map::new();
            params.insert("timeout_ms".into(), json!(arg_f64(args, 0)? as i64));
            Ok(Plan::request("wait_for_stabilize", params))
        }
        "m.describe" => Ok(Plan::request("describe", Map::new())),
        "m.status" => {
            let mut params = Map::new();
            params.insert("action_id".into(), json!(arg_str(args, 0)?));
            Ok(Plan::request("status", params))
        }
        other => Err(NsError::UndeclaredMethod(other.to_string())),
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn base64_decode(text: &str) -> Result<Vec<u8>, NsError> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .map_err(|e| NsError::Contract(format!("wire field is not valid base64: {e}")))
}

/// After the wire answers: shape the result for the worker and fold any new
/// capture geometry into `frame`.
fn finish(
    method: &str,
    mut result: Value,
    frame: &Arc<Mutex<Frame>>,
    shot_dir: &std::path::Path,
) -> Result<Value, NsError> {
    match method {
        "m.observe" => {
            let obj = result
                .as_object_mut()
                .ok_or_else(|| NsError::Contract("observe must answer with an object".into()))?;
            let width = obj.get("width").and_then(Value::as_i64).unwrap_or(0);
            let height = obj.get("height").and_then(Value::as_i64).unwrap_or(0);
            let rotation = obj.get("rotation").and_then(Value::as_i64).unwrap_or(0);
            let revision = obj.get("revision").cloned().unwrap_or(Value::Null);
            *frame.lock().expect("frame poisoned") = Frame {
                width,
                height,
                rotation,
            };
            // One look, one capture (inventory §3.2): bind what this
            // observation saw — tree revision + geometry — so the one-use id
            // the host mints names a specific view, not a bare counter.
            obj.insert(
                CAPTURE_KEY.into(),
                json!({
                    "revision": revision,
                    "width": width,
                    "height": height,
                    "rotation": rotation,
                }),
            );
            // The executor base64s the screenshot into `png`; the multimodal
            // contract wants image wire. Land it on disk (like b.screenshot)
            // so ImageHandle stays path-based.
            if let Some(Value::String(png)) = obj.remove("png") {
                let bytes = base64_decode(&png)?;
                let path = shot_dir.join(format!(
                    "m-observe-{}.png",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or_default()
                ));
                std::fs::write(&path, &bytes).map_err(|e| {
                    NsError::Contract(format!("cannot write {}: {e}", path.display()))
                })?;
                obj.insert(
                    "images".into(),
                    json!([{
                        "path": path.display().to_string(),
                        "mime": "image/png",
                        "alt": format!("phone screen {width}x{height}"),
                    }]),
                );
            }
            Ok(result)
        }
        "m.shell" => {
            // stdout crosses base64; the model wants text.
            if let Some(obj) = result.as_object_mut()
                && let Some(encoded) = obj.get("stdout").and_then(Value::as_str)
            {
                let bytes = base64_decode(encoded)?;
                obj.insert(
                    "stdout".into(),
                    json!(String::from_utf8_lossy(&bytes).into_owned()),
                );
            }
            Ok(result)
        }
        "m.file_read" => {
            if let Some(obj) = result.as_object_mut()
                && let Some(encoded) = obj.get("data").and_then(Value::as_str)
            {
                let bytes = base64_decode(encoded)?;
                obj.insert(
                    "text".into(),
                    json!(String::from_utf8_lossy(&bytes).into_owned()),
                );
                obj.insert("bytes".into(), json!(bytes.len()));
                obj.remove("data");
            }
            Ok(result)
        }
        _ => Ok(result),
    }
}

/// One host-side `m.*` call: plan it, send it, shape the answer.
struct MCall {
    wire: Option<Arc<dyn Transport>>,
    frame: Arc<Mutex<Frame>>,
    shot_dir: Arc<std::path::PathBuf>,
    method: &'static str,
}

#[async_trait]
impl HostCall for MCall {
    async fn call(&self, args: Vec<Value>) -> Result<Value, NsError> {
        let wire = self.wire.as_ref().ok_or_else(|| {
            NsError::Transport(
                "m.* is not connected — the pack is assembled without a wire (config-level plug)"
                    .into(),
            )
        })?;
        let frame = self.frame.lock().expect("frame poisoned").clone();
        let mut plan = plan(self.method, &args, &frame)?;
        let (op, params) = match &mut plan {
            Plan::Request { op, params } => (op.clone(), Value::Object(std::mem::take(params))),
            Plan::ScrollPoint {
                x,
                y,
                direction,
                amount,
            } => {
                // mu's own point-scroll trajectory (`Vision.scroll`): a short
                // drag centered on the point, oriented by direction.
                if frame.width <= 0 || frame.height <= 0 {
                    return Err(NsError::Contract(
                        "m.scroll on a point needs a capture first — call m.observe".into(),
                    ));
                }
                let (x, y, direction, amount) = (*x, *y, direction.clone(), *amount);
                let horizontal = matches!(direction.as_str(), "left" | "right");
                let extent = (if horizontal {
                    frame.width
                } else {
                    frame.height
                } - 1) as f64;
                let delta = 0.8 * amount * extent;
                let sign_down = matches!(direction.as_str(), "down" | "right");
                let center = if horizontal { x } else { y };
                let low = (center - delta / 2.0).clamp(0.0, (extent - delta).max(0.0));
                let (a, b) = if sign_down {
                    (low + delta, low)
                } else {
                    (low, low + delta)
                };
                let mut params = Map::new();
                params.insert("action".into(), json!("point_scroll"));
                params.insert("x".into(), json!(if horizontal { a } else { x }));
                params.insert("y".into(), json!(if horizontal { y } else { a }));
                params.insert("end_x".into(), json!(if horizontal { b } else { x }));
                params.insert("end_y".into(), json!(if horizontal { y } else { b }));
                params.insert("duration_ms".into(), json!(500));
                params.insert("frame_width".into(), json!(frame.width));
                params.insert("frame_height".into(), json!(frame.height));
                params.insert("frame_rotation".into(), json!(frame.rotation));
                ("action".to_string(), Value::Object(params))
            }
        };
        let result = wire.request(&op, params).await?;
        finish(self.method, result, &self.frame, &self.shot_dir)
    }
}

/// The `m.*` capability pack: one declared face over one wire channel.
pub struct PhiNsMobile {
    wire: Option<Arc<dyn Transport>>,
    frame: Arc<Mutex<Frame>>,
    shot_dir: Arc<std::path::PathBuf>,
}

impl PhiNsMobile {
    /// Connected pack: every call goes out over `wire`.
    pub fn new(wire: Arc<dyn Transport>) -> Self {
        Self {
            wire: Some(wire),
            frame: Arc::new(Mutex::new(Frame::default())),
            shot_dir: Arc::new(
                std::env::temp_dir().join(format!("phi-ns-mobile-{}", std::process::id())),
            ),
        }
    }

    /// Declaration-only pack (docs, contract tests): calls fail with
    /// "not connected" instead of hanging or inventing results.
    pub fn unconnected() -> Self {
        Self {
            wire: None,
            frame: Arc::new(Mutex::new(Frame::default())),
            shot_dir: Arc::new(
                std::env::temp_dir().join(format!("phi-ns-mobile-{}", std::process::id())),
            ),
        }
    }

    /// Where observation screenshots land.
    pub fn with_shot_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.shot_dir = Arc::new(dir.into());
        self
    }
}

#[async_trait]
impl NamespaceProvider for PhiNsMobile {
    fn namespaces(&self) -> &'static [&'static str] {
        &["m"]
    }

    fn methods(&self) -> Vec<MethodSpec> {
        methods::method_table()
    }

    async fn install(&self, sink: &mut dyn WorkerSink) -> Result<(), NsError> {
        for spec in self.methods() {
            sink.register_fn(
                spec.name,
                Box::new(MCall {
                    wire: self.wire.clone(),
                    frame: Arc::clone(&self.frame),
                    shot_dir: Arc::clone(&self.shot_dir),
                    method: spec.name,
                }),
            )?;
        }
        sink.eval(PY_SHIM).await
    }

    fn summarize(&self, _name: &str, obj: &NsObject) -> Option<String> {
        // Only the shim's own classes carry `_phi_summary_`; the engine's
        // generic fallback covers model-built names.
        if matches!(
            obj.kind.as_str(),
            "MobileNs" | "Obs" | "Node" | "ImageHandle"
        ) {
            Some(obj.detail.clone().unwrap_or_else(|| obj.repr.clone()))
        } else {
            None
        }
    }

    async fn on_session_end(&self) -> Result<(), NsError> {
        // kill-on-drop owns the stub; a clean stop records a real exit.
        Ok(())
    }
}

/// Wire request built from a plan — exposed for the pack's tests to pin the
/// method → wire mapping without a live broker.
pub fn debug_plan(method: &str, args: &[Value]) -> Result<(String, Value), NsError> {
    let plan = plan(method, args, &Frame::default())?;
    match plan {
        Plan::Request { op, params } => Ok((op, Value::Object(params))),
        Plan::ScrollPoint { .. } => Err(NsError::Contract(
            "point scroll needs capture geometry (not available in debug_plan)".into(),
        )),
    }
}
