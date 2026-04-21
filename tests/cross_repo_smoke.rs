use std::fs;
use std::path::PathBuf;

use hibana::substrate::{
    policy::PolicySlot,
    tap::TapEvent,
    transport::{TransportSnapshot, TransportSnapshotParts},
};
use hibana_epf::{
    Action, Header, HostSlots, PolicyAnnotation, ROLE_CLUSTER as EPF_ROLE_CLUSTER,
    ROLE_CONTROLLER as EPF_ROLE_CONTROLLER, ScratchLease, Slot, loader::ImageLoader, run_with,
};
use hibana_mgmt::{LoadRequest, Request, SubscribeReq};

fn sibling_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cross-repo harness has parent")
        .join(path)
}

fn read_sibling(path: &str) -> String {
    let full = sibling_path(path);
    fs::read_to_string(&full)
        .unwrap_or_else(|err| panic!("read {} failed: {}", full.display(), err))
}

fn header_for(code: &[u8], mem_len: u16) -> Header {
    Header {
        code_len: code.len() as u16,
        fuel_max: 8,
        mem_len,
        flags: 0,
        hash: hibana_epf::verifier::compute_hash(code),
    }
}

#[test]
fn mgmt_surface_uses_attach_helpers_without_raw_program_exports() {
    for path in [
        "hibana-mgmt/src/request_reply.rs",
        "hibana-mgmt/src/observe_stream.rs",
    ] {
        let src = read_sibling(path);
        assert!(src.contains("pub fn attach_controller"));
        assert!(src.contains("pub fn attach_cluster"));
        assert!(!src.contains("pub const PROGRAM"));
        assert!(!src.contains("pub const PREFIX"));
        assert!(!src.contains("g::advanced::steps"));
        assert!(!src.contains("const APP: g::Program<_>"));
        assert!(!src.contains("static APP: g::Program<_>"));
        assert!(!src.contains("const PROGRAM: g::Program<_>"));
        assert!(!src.contains("static PROGRAM: g::Program<_>"));
        assert!(!src.contains("project(&PROGRAM)"));
        assert!(!src.contains("project::<"));
    }

    let request_reply = read_sibling("hibana-mgmt/src/request_reply.rs");
    assert!(request_reply.contains("GenericCapToken<LoadBeginKind>"));
    assert!(request_reply.contains("GenericCapToken<LoadCommitKind>"));
    assert!(!request_reply.contains("Msg<LABEL_MGMT_LOAD_BEGIN,"));
    assert!(!request_reply.contains("Msg<LABEL_MGMT_LOAD_COMMIT,"));

    let _request = Request::LoadAndActivate(LoadRequest {
        slot: PolicySlot::Route,
        code: &[0x30, 0x03, 0x00, 0x01],
        fuel_max: 64,
        mem_len: 128,
    });
    let _subscribe = SubscribeReq::default();
}

#[test]
fn epf_surface_exposes_lifecycle_attach_helpers() {
    let src = read_sibling("hibana-epf/src/lib.rs");
    assert!(src.contains("pub fn attach_controller"));
    assert!(src.contains("pub fn attach_cluster"));
    assert!(!src.contains("pub const PROGRAM"));
    assert!(!src.contains("pub const PREFIX"));
    assert!(!src.contains("g::advanced::steps"));
    assert!(!src.contains("const APP: g::Program<_>"));
    assert!(!src.contains("static APP: g::Program<_>"));
    assert!(!src.contains("const PROGRAM: g::Program<_>"));
    assert!(!src.contains("static PROGRAM: g::Program<_>"));
    assert!(!src.contains("project(&PROGRAM)"));
    assert!(!src.contains("project::<"));

    let kinds = read_sibling("hibana-epf/src/control_kinds.rs");
    assert!(kinds.contains("pub struct PolicyLoadKind;"));
    assert!(kinds.contains("pub struct PolicyActivateKind;"));
    assert!(kinds.contains("pub struct PolicyRevertKind;"));
    assert!(kinds.contains("pub struct PolicyAnnotateKind;"));
    assert!(src.contains("GenericCapToken<PolicyLoadKind>"));
    assert!(src.contains("GenericCapToken<PolicyActivateKind>"));
    assert!(src.contains("GenericCapToken<PolicyRevertKind>"));
    assert!(src.contains("GenericCapToken<PolicyAnnotateKind>"));
    assert!(!src.contains("Msg<LABEL_POLICY_LOAD, u32>"));
    assert!(!src.contains("Msg<LABEL_POLICY_ACTIVATE, u8>"));
    assert!(!src.contains("Msg<LABEL_POLICY_REVERT, u8>"));
    assert!(!src.contains("Msg<LABEL_POLICY_ANNOTATE, PolicyAnnotation>"));

    let _ = (EPF_ROLE_CONTROLLER, EPF_ROLE_CLUSTER);
    let _annotation = PolicyAnnotation { digest: 7 };
}

#[test]
fn epf_runtime_executes_under_split_repo_dependency_shape() {
    let code = [0x41, 0x00, 0x33, 0x00];
    let mut loader = ImageLoader::new();
    loader.begin(header_for(&code, 16)).expect("begin");
    loader.write(0, &code).expect("write");
    let verified = loader.commit_for_slot(Slot::Route).expect("verify");

    let mut slots = HostSlots::new();
    let mut scratch = [0u8; 16];
    slots
        .install_verified(Slot::Route, verified, ScratchLease::new(&mut scratch))
        .expect("install");

    let action = run_with(
        &slots,
        Slot::Route,
        &TapEvent::zero(),
        None,
        None,
        |ctx| {
            ctx.set_transport_snapshot(TransportSnapshot::from_parts(TransportSnapshotParts {
                queue_depth: Some(3),
                ..TransportSnapshotParts::new()
            }))
        },
    );
    assert_eq!(action, Action::Route { arm: 3 });
}
