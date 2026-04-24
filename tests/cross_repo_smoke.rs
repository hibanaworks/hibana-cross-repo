use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use hibana::substrate::{
    policy::{ContextValue, PolicyAttrs, PolicySlot, core as policy_core},
    tap::TapEvent,
};
use hibana_epf::{
    Action, ENGINE_FAIL_CLOSED, Header, HostSlots, PolicyAnnotation,
    ROLE_CONTROLLER as EPF_ROLE_CONTROLLER, ScratchLease, Slot, loader::ImageLoader, run_with,
};
use hibana_mgmt::{LoadRequest, Request, SubscribeReq};

const WORKSPACE_PATCH_SENTINEL: &str = "run_workspace_smoke.sh";

fn manifest() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {} failed: {}", path.display(), err))
}

fn lockfile() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.lock");
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {} failed: {}", path.display(), err))
}

fn workspace_smoke_mode() -> bool {
    workspace_patch_enabled(
        std::env::var_os("HIBANA_CROSS_REPO_WORKSPACE_SMOKE"),
        std::env::var_os("HIBANA_CROSS_REPO_WORKSPACE_PATCHED"),
    )
}

fn workspace_patch_enabled(smoke: Option<OsString>, patched: Option<OsString>) -> bool {
    smoke.is_some() && patched.as_deref() == Some(std::ffi::OsStr::new(WORKSPACE_PATCH_SENTINEL))
}

fn workspace_repo_root(repo: &str) -> PathBuf {
    let key = match repo {
        "hibana" => "HIBANA_CROSS_REPO_HIBANA_DIR",
        "hibana-epf" => "HIBANA_CROSS_REPO_HIBANA_EPF_DIR",
        "hibana-mgmt" => "HIBANA_CROSS_REPO_HIBANA_MGMT_DIR",
        _ => panic!("unknown cross-repo sibling: {repo}"),
    };
    std::env::var_os(key)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("workspace smoke script did not provide {key}"))
}

fn sibling_path(path: &str) -> PathBuf {
    let (repo, rest) = path
        .split_once('/')
        .unwrap_or_else(|| panic!("sibling source path must start with repo name: {path}"));
    workspace_repo_root(repo).join(rest)
}

fn read_sibling_workspace_only(path: &str) -> String {
    assert!(
        workspace_smoke_mode(),
        "local sibling source reads are only valid through run_workspace_smoke.sh patch mode"
    );
    let full = sibling_path(path);
    fs::read_to_string(&full)
        .unwrap_or_else(|err| panic!("read {} failed: {}", full.display(), err))
}

fn header_for(code: &[u8], mem_len: u16) -> Header {
    Header {
        code_len: code.len() as u16,
        fuel_max: 8,
        mem_len,
        hash: hibana_epf::verifier::compute_hash(code),
    }
}

fn queue_depth_attrs(queue_depth: u32) -> PolicyAttrs {
    let mut attrs = PolicyAttrs::new();
    assert!(
        attrs.insert(
            policy_core::QUEUE_DEPTH,
            ContextValue::from_u32(queue_depth),
        ),
        "queue depth attr must fit in PolicyAttrs"
    );
    attrs
}

#[test]
fn workspace_source_read_mode_requires_script_patch_sentinel() {
    let smoke = Some(OsString::from("1"));
    let sentinel = Some(OsString::from(WORKSPACE_PATCH_SENTINEL));

    assert!(!workspace_patch_enabled(smoke.clone(), None));
    assert!(!workspace_patch_enabled(None, sentinel.clone()));
    assert!(!workspace_patch_enabled(
        smoke.clone(),
        Some(OsString::from("manual"))
    ));
    assert!(workspace_patch_enabled(smoke, sentinel));
}

#[test]
fn mgmt_surface_uses_attach_helpers_without_raw_program_exports() {
    if !workspace_smoke_mode() {
        return;
    }

    for path in [
        "hibana-mgmt/src/request_reply.rs",
        "hibana-mgmt/src/observe_stream.rs",
    ] {
        let src = read_sibling_workspace_only(path);
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

    let request_reply = read_sibling_workspace_only("hibana-mgmt/src/request_reply.rs");
    assert!(request_reply.contains("GenericCapToken<LoadBeginKind>"));
    assert!(request_reply.contains("GenericCapToken<LoadCommitKind>"));
    assert!(request_reply.contains("LABEL_MGMT_REVERT"));
    assert!(!request_reply.contains("LABEL_MGMT_RESTORE"));
    assert!(!request_reply.contains("Restore"));
    assert!(!request_reply.contains("Restored"));
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
fn manifest_default_lane_tracks_exact_git_revs() {
    let cargo_toml = manifest();
    assert!(cargo_toml.contains("git = \"https://github.com/hibanaworks/hibana\""));
    assert!(cargo_toml.contains("git = \"https://github.com/hibanaworks/hibana-mgmt\""));
    assert!(cargo_toml.contains("git = \"https://github.com/hibanaworks/hibana-epf\""));
    assert!(cargo_toml.contains("rev = \"ab2f2c90b04d9b80c97c6c69b864452463ee6df5\""));
    assert!(cargo_toml.contains("rev = \"30a49a3caced9a92a94b1f2239e5f85b60ee9013\""));
    assert!(cargo_toml.contains("rev = \"02c654f587c82b11445bb612e38f57d477a602e7\""));
    assert!(!cargo_toml.contains("path = \"../hibana\""));
    assert!(!cargo_toml.contains("path = \"../hibana-mgmt\""));
    assert!(!cargo_toml.contains("path = \"../hibana-epf\""));
}

#[test]
fn lockfile_pins_resolved_git_sources() {
    if workspace_smoke_mode() {
        return;
    }
    let cargo_lock = lockfile();
    assert!(cargo_lock.contains(
        "source = \"git+https://github.com/hibanaworks/hibana?rev=ab2f2c90b04d9b80c97c6c69b864452463ee6df5#ab2f2c90b04d9b80c97c6c69b864452463ee6df5\""
    ));
    assert!(cargo_lock.contains(
        "source = \"git+https://github.com/hibanaworks/hibana-mgmt?rev=02c654f587c82b11445bb612e38f57d477a602e7#02c654f587c82b11445bb612e38f57d477a602e7\""
    ));
    assert!(cargo_lock.contains(
        "source = \"git+https://github.com/hibanaworks/hibana-epf?rev=30a49a3caced9a92a94b1f2239e5f85b60ee9013#30a49a3caced9a92a94b1f2239e5f85b60ee9013\""
    ));
}

#[test]
fn epf_surface_exposes_controller_lifecycle_attach_only() {
    if !workspace_smoke_mode() {
        return;
    }

    let src = read_sibling_workspace_only("hibana-epf/src/lib.rs");
    assert!(src.contains("pub fn attach_controller"));
    assert!(!src.contains("pub fn attach_cluster"));
    assert!(!src.contains("ROLE_CLUSTER"));
    assert!(!src.contains("pub const PROGRAM"));
    assert!(!src.contains("pub const PREFIX"));
    assert!(!src.contains("g::advanced::steps"));
    assert!(!src.contains("const APP: g::Program<_>"));
    assert!(!src.contains("static APP: g::Program<_>"));
    assert!(!src.contains("const PROGRAM: g::Program<_>"));
    assert!(!src.contains("static PROGRAM: g::Program<_>"));
    assert!(!src.contains("project(&PROGRAM)"));
    assert!(!src.contains("project::<"));

    let kinds = read_sibling_workspace_only("hibana-epf/src/control_kinds.rs");
    assert!(kinds.contains("pub struct PolicyLoadKind;"));
    assert!(kinds.contains("pub struct PolicyActivateKind;"));
    assert!(kinds.contains("pub struct PolicyRevertKind;"));
    assert!(!kinds.contains("pub struct PolicyRestoreKind;"));
    assert!(kinds.contains("pub struct PolicyAnnotateKind;"));
    assert!(src.contains("GenericCapToken<PolicyLoadKind>"));
    assert!(src.contains("GenericCapToken<PolicyActivateKind>"));
    assert!(src.contains("GenericCapToken<PolicyRevertKind>"));
    assert!(!src.contains("GenericCapToken<PolicyRestoreKind>"));
    assert!(src.contains("GenericCapToken<PolicyAnnotateKind>"));
    assert!(!src.contains("Msg<LABEL_POLICY_LOAD, u32>"));
    assert!(!src.contains("Msg<LABEL_POLICY_ACTIVATE, u8>"));
    assert!(!src.contains("Msg<LABEL_POLICY_RESTORE, u8>"));
    assert!(!src.contains("Msg<LABEL_POLICY_ANNOTATE, PolicyAnnotation>"));

    let _ = EPF_ROLE_CONTROLLER;
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

    let action = run_with(&slots, Slot::Route, &TapEvent::zero(), None, None, |ctx| {
        ctx.set_policy_attrs(queue_depth_attrs(1))
    });
    assert_eq!(action, Action::Route { arm: 1 });
}

#[test]
fn epf_runtime_rejects_non_binary_route_arm_under_split_repo_dependency_shape() {
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

    let action = run_with(&slots, Slot::Route, &TapEvent::zero(), None, None, |ctx| {
        ctx.set_policy_attrs(queue_depth_attrs(3))
    });
    assert!(matches!(
        action,
        Action::Abort(info) if info.reason == ENGINE_FAIL_CLOSED && info.trap.is_none()
    ));
}
