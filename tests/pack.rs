//! The `m.*` declaration source and its wire mapping (O10 inventory §2).
//!
//! Two things must hold, and only tests keep them honest: the declared face
//! matches the inventory's decision boundary (act verbs ask, observe verbs
//! flow), and the method → wire mapping matches what the Android executor
//! actually dispatches (`Main.java`) — not what the Python surface suggests.

use async_trait::async_trait;
use phi_cap::{Capability, CapabilitySpec};
use phi_ns::remote::Transport;
use phi_ns::{HostCall, MethodSpec, NamespaceProvider, NsError, NsObject, WorkerSink};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Mutex;

phi_ns::provider_contract_tests!(phi_ns_mobile::PhiNsMobile::unconnected());

fn b64(text: &str) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(text.as_bytes())
}

// ---- fake sink + wire, to drive the real install() → HostCall path ----

#[derive(Default)]
struct FakeSink {
    fns: BTreeMap<String, Box<dyn HostCall>>,
}

#[async_trait]
impl WorkerSink for FakeSink {
    fn register_fn(&mut self, name: &str, call: Box<dyn HostCall>) -> Result<(), NsError> {
        self.fns.insert(name.to_string(), call);
        Ok(())
    }

    async fn eval(&mut self, _code: &str) -> Result<(), NsError> {
        Ok(())
    }

    async fn namespace_entries(&mut self) -> Result<Vec<NsObject>, NsError> {
        Ok(Vec::new())
    }
}

impl FakeSink {
    async fn invoke(&self, method: &str, args: Vec<Value>) -> Result<Value, NsError> {
        self.fns
            .get(method)
            .unwrap_or_else(|| panic!("{method} not installed"))
            .call(args)
            .await
    }
}

#[derive(Default)]
struct MockWire {
    seen: Mutex<Vec<(String, Value)>>,
    reply: Mutex<Value>,
}

impl MockWire {
    fn answering(reply: Value) -> Self {
        Self {
            seen: Mutex::new(Vec::new()),
            reply: Mutex::new(reply),
        }
    }

    fn last(&self) -> (String, Value) {
        self.seen
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("no wire request was sent")
    }
}

#[async_trait]
impl Transport for MockWire {
    async fn request(&self, method: &str, args: Value) -> Result<Value, NsError> {
        self.seen.lock().unwrap().push((method.to_string(), args));
        Ok(self.reply.lock().unwrap().clone())
    }
}

/// Install the pack over one mock wire; `dir` is where screenshots land.
async fn installed(reply: Value, dir: &std::path::Path) -> (FakeSink, std::sync::Arc<MockWire>) {
    std::fs::create_dir_all(dir).unwrap();
    let wire = std::sync::Arc::new(MockWire::answering(reply));
    let pack = phi_ns_mobile::PhiNsMobile::new(wire.clone()).with_shot_dir(dir);
    let mut sink = FakeSink::default();
    pack.install(&mut sink).await.expect("install");
    (sink, wire)
}

fn scratch_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("phi-ns-mobile-test-{}-{}", tag, std::process::id()))
}

// ---- declaration shape ----

fn table() -> Vec<MethodSpec> {
    phi_ns_mobile::PhiNsMobile::unconnected().methods()
}

#[test]
fn the_declaration_source_is_the_20_method_face() {
    let methods = table();
    assert_eq!(methods.len(), 20, "the m.* face is 20 methods");
    for m in &methods {
        assert!(m.name.starts_with("m."), "{} is not rooted at m", m.name);
        assert!(!m.doc.trim().is_empty(), "{} has no doc", m.name);
    }
    let multimodal: Vec<&str> = methods
        .iter()
        .filter(|m| m.multimodal)
        .map(|m| m.name)
        .collect();
    assert_eq!(
        multimodal,
        ["m.observe"],
        "only m.observe returns pictures (vision mode)"
    );
    let doc = phi_ns_mobile::PhiNsMobile::unconnected().api_doc();
    assert_eq!(doc.lines().count(), 20, "one seed line per method:\n{doc}");
}

#[test]
fn the_inventory_decision_columns_partition_the_face() {
    // Inventory §5, the columns MethodPolicy's initial rows are generated
    // from. The pack owns the names, policy owns the labels; this pins the
    // names each label was drawn for, so a rename that silently changes a
    // decision boundary fails here first.
    const FLOW: &[&str] = &["m.observe", "m.apps", "m.file_read"];
    const ASK: &[&str] = &[
        "m.click",
        "m.scroll",
        "m.swipe",
        "m.long_press",
        "m.drag",
        "m.set_text",
        "m.select_text",
        "m.perform",
        "m.text",
        "m.press_key",
        "m.system",
        "m.launch",
        "m.file_write",
        "m.shell",
    ];
    const ALLOW: &[&str] = &["m.stabilize", "m.describe", "m.status"];
    let names: Vec<&str> = table().iter().map(|m| m.name).collect();
    for name in FLOW.iter().chain(ASK).chain(ALLOW) {
        assert!(names.contains(name), "{name} dropped out of the face");
    }
    assert_eq!(FLOW.len() + ASK.len() + ALLOW.len(), names.len());
}

#[test]
fn the_envelope_matches_the_decision_on_mobile() {
    // Here the two agree: what the method touches decides whether it asks.
    // (On the desktop face they deliberately do not — agent_cursor.)
    const ASK: &[&str] = &[
        "m.click",
        "m.scroll",
        "m.swipe",
        "m.long_press",
        "m.drag",
        "m.set_text",
        "m.select_text",
        "m.perform",
        "m.text",
        "m.press_key",
        "m.system",
        "m.launch",
        "m.file_write",
        "m.shell",
    ];
    for m in table() {
        let is_input = m
            .caps
            .iter()
            .any(|c| matches!(c, CapabilitySpec::DeviceInput(_)));
        assert_eq!(
            is_input,
            ASK.contains(&m.name),
            "{}'s capability envelope does not match its decision row",
            m.name
        );
        if is_input {
            assert_eq!(m.capability(), Capability::Device);
        }
    }
}

#[test]
fn the_desktop_mouse_verbs_are_deliberately_undeclared() {
    // Inventory §2.1: these land as UNSUPPORTED_CAPABILITY on Android, and a
    // seed line that cannot land is a lie (declared = enforceable).
    for gone in [
        "m.move",
        "m.button_down",
        "m.button_up",
        "m.wheel",
        "m.key_down",
        "m.key_up",
        "m.release_all",
    ] {
        assert!(
            table().iter().all(|m| m.name != gone),
            "{gone} must not be declared for the Android face"
        );
    }
}

// ---- method → wire mapping (Main.java's dispatch table) ----

#[test]
fn observe_maps_to_the_wire_capture_op() {
    let ax = phi_ns_mobile::debug_plan("m.observe", &[json!("ax"), Value::Null]).unwrap();
    assert_eq!(ax.0, "observe");
    assert_eq!(ax.1["tree"], true);
    assert_eq!(ax.1["screenshot"], false);

    let vision = phi_ns_mobile::debug_plan("m.observe", &[json!("vision"), Value::Null]).unwrap();
    assert_eq!(vision.1["tree"], false);
    assert_eq!(vision.1["screenshot"], true);
}

#[test]
fn node_clicks_and_point_clicks_pick_different_wire_actions() {
    let node = phi_ns_mobile::debug_plan(
        "m.click",
        &[json!({"node_id": "n7", "revision": "3", "target_token": "t"})],
    )
    .unwrap();
    assert_eq!(node.0, "action");
    assert_eq!(node.1["action"], "click");
    assert_eq!(node.1["node_id"], "n7");
    assert_eq!(node.1["revision"], "3");

    let point = phi_ns_mobile::debug_plan("m.click", &[json!([10, 20])]).unwrap();
    assert_eq!(point.1["action"], "point_click");
    assert_eq!(point.1["x"], 10.0);
    assert_eq!(point.1["y"], 20.0);
}

#[test]
fn file_access_splits_at_the_decision_boundary() {
    let read =
        phi_ns_mobile::debug_plan("m.file_read", &[json!("/sdcard/a.txt"), json!("read")]).unwrap();
    assert_eq!(read.0, "file");
    assert_eq!(read.1["operation"], "read");
    assert!(
        phi_ns_mobile::debug_plan("m.file_read", &[json!("/x"), json!("write")]).is_err(),
        "reads cannot smuggle a write through the file face"
    );

    let write = phi_ns_mobile::debug_plan(
        "m.file_write",
        &[
            json!("write"),
            json!("/sdcard/b.txt"),
            json!({"data": "hi"}),
        ],
    )
    .unwrap();
    assert_eq!(write.0, "file");
    assert_eq!(write.1["operation"], "write");
    // text content crosses base64 — the wire contract for `data`
    assert_eq!(write.1["data"], b64("hi"));
}

#[test]
fn perform_maps_action_names_to_platform_constants() {
    let p = phi_ns_mobile::debug_plan(
        "m.perform",
        &[json!({"node_id": "n1", "revision": "1"}), json!("expand")],
    )
    .unwrap();
    assert_eq!(p.1["action"], "perform");
    assert_eq!(p.1["node_action"], 262144); // ACTION_EXPAND
    assert!(
        phi_ns_mobile::debug_plan(
            "m.perform",
            &[json!({"node_id": "n1", "revision": "1"}), json!("teleport")]
        )
        .is_err(),
        "unknown action names must fail, not guess"
    );
}

#[test]
fn launch_carries_the_app_marker_the_executor_strips() {
    let p = phi_ns_mobile::debug_plan("m.launch", &[json!("com.example.app")]).unwrap();
    assert_eq!(p.1["action"], "launch");
    assert_eq!(p.1["app_id"], "app:com.example.app");
}

#[test]
fn press_key_sends_portable_names_for_the_broker_to_translate() {
    let p = phi_ns_mobile::debug_plan("m.press_key", &[json!("enter")]).unwrap();
    assert_eq!(p.1["action"], "press_key");
    assert_eq!(p.1["key"], "enter");
    assert!(
        p.1.get("key_code").is_none(),
        "key codes are the broker's business, not ours"
    );
}

// ---- end-to-end through install() → HostCall → wire ----

#[tokio::test]
async fn observe_folds_the_screenshot_into_the_multimodal_contract() {
    let dir = scratch_dir("observe");
    let (sink, _wire) = installed(
        json!({
            "nodes": [{"id": "n1", "role": "text", "text": "Settings", "target_token": "t1"}],
            "revision": "8", "width": 1080, "height": 2400, "png": b64("fake-png-bytes"),
        }),
        &dir,
    )
    .await;

    let out = sink
        .invoke("m.observe", vec![json!("vision"), Value::Null])
        .await
        .expect("observe answers");
    assert_eq!(out["revision"], "8");
    assert!(
        out.get("png").is_none(),
        "raw base64 must not reach the worker"
    );
    let images = out["images"]
        .as_array()
        .expect("composite return carries images");
    assert_eq!(images.len(), 1);
    let path = images[0]["path"].as_str().unwrap();
    assert!(
        std::path::Path::new(path).exists(),
        "screenshot landed on disk"
    );
    assert_eq!(images[0]["mime"], "image/png");
    assert_eq!(images[0]["alt"], "phone screen 1080x2400");
    // the harvest contract must see exactly this
    let harvested = phi_ns::ImageRef::from_wire(&out).expect("harvest");
    assert_eq!(harvested.len(), 1);
}

#[tokio::test]
async fn a_point_scroll_computes_mu_trajectory_from_the_last_capture() {
    let dir = scratch_dir("scroll");
    let (sink, wire) = installed(
        json!({"nodes": [], "revision": "1", "width": 101, "height": 201}),
        &dir,
    )
    .await;
    sink.invoke("m.observe", vec![json!("ax"), Value::Null])
        .await
        .expect("observe");

    sink.invoke(
        "m.scroll",
        vec![json!([50.0, 100.0]), json!("down"), json!(1.0)],
    )
    .await
    .expect("scroll");
    let (op, params) = wire.last();
    assert_eq!(op, "action");
    assert_eq!(params["action"], "point_scroll");
    // mu's Vision.scroll: delta = 0.8 * amount * (extent - 1); a downward
    // flick drags from below to above. Frame 101x201 → vertical extent 200.
    assert_eq!(params["frame_height"], 201);
    let y = params["y"].as_f64().unwrap();
    let end_y = params["end_y"].as_f64().unwrap();
    assert!(
        (y - end_y - 160.0).abs() < 1e-9,
        "trajectory length is 0.8*(201-1), got {} → {}",
        y,
        end_y
    );
    assert!(y > end_y, "downward scroll drags from below to above");
}

#[tokio::test]
async fn node_actions_carry_the_revision_that_invalidates_them() {
    let dir = scratch_dir("node");
    let (sink, wire) = installed(json!({"ok": true}), &dir).await;
    sink.invoke(
        "m.click",
        vec![json!({"node_id": "n3", "revision": "7", "target_token": "tk"})],
    )
    .await
    .expect("click");
    let (op, params) = wire.last();
    assert_eq!(op, "action");
    assert_eq!(params["action"], "click");
    assert_eq!(params["node_id"], "n3");
    assert_eq!(params["revision"], "7");
    assert_eq!(params["target_token"], "tk");
}

#[tokio::test]
async fn binary_wire_fields_come_back_as_text() {
    let dir = scratch_dir("text");
    let (sink, _wire) = installed(
        json!({
            "state": "exited", "exit_code": 0, "stdout": b64("hello\n"),
        }),
        &dir,
    )
    .await;
    let out = sink
        .invoke("m.shell", vec![json!(["echo", "hello"]), json!(10000)])
        .await
        .expect("shell");
    assert_eq!(out["stdout"], "hello\n");
    assert_eq!(out["exit_code"], 0);

    let (sink, _wire) = installed(
        json!({"path": "/sdcard/a.txt", "data": b64("file-body")}),
        &dir,
    )
    .await;
    let out = sink
        .invoke("m.file_read", vec![json!("/sdcard/a.txt"), json!("read")])
        .await
        .expect("file_read");
    assert_eq!(out["text"], "file-body");
    assert_eq!(out["bytes"], 9);
    assert!(out.get("data").is_none(), "base64 payload must not leak");
}

#[tokio::test]
async fn an_unconnected_pack_fails_instead_of_hanging() {
    let pack = phi_ns_mobile::PhiNsMobile::unconnected();
    let mut sink = FakeSink::default();
    pack.install(&mut sink).await.expect("install");
    let err = sink
        .invoke("m.describe", vec![])
        .await
        .expect_err("no wire, no answer");
    assert!(
        matches!(err, NsError::Transport(_)),
        "expected Transport, got {err:?}"
    );
}
