use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use hibana::substrate::tap::TapEvent;
use hibana_epf::{
    Action, ENGINE_FAIL_CLOSED, Header, HostSlots, ROLE_CONTROLLER as EPF_ROLE_CONTROLLER,
    ScratchLease, loader::ImageLoader, run_with, vm::Slot,
};
use hibana_mgmt::SubscribeReq;

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

fn manifest_rev(cargo_toml: &str, crate_name: &str) -> String {
    let needle = format!(
        "{crate_name} = {{ git = \"https://github.com/hibanaworks/{crate_name}\", rev = \""
    );
    let start = cargo_toml
        .find(&needle)
        .unwrap_or_else(|| panic!("manifest must pin {crate_name} to a GitHub rev"))
        + needle.len();
    let rev = cargo_toml[start..]
        .split('"')
        .next()
        .unwrap_or_else(|| panic!("manifest rev for {crate_name} must be quoted"));
    assert_eq!(
        rev.len(),
        40,
        "manifest rev for {crate_name} must be a full commit SHA"
    );
    assert!(
        rev.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "manifest rev for {crate_name} must be hex"
    );
    rev.to_owned()
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

fn assert_no_old_surface_paths(path: &str, src: &str) {
    for forbidden in [
        "g::advanced",
        "hibana::g::advanced",
        "hibana::substrate::Lane",
        "hibana::substrate::SessionId",
        "hibana::substrate::RendezvousId",
        "use hibana::substrate::{\n    Lane",
        "use hibana::substrate::{Lane",
        "use hibana::substrate::{\n    SessionId",
        "use hibana::substrate::{SessionId",
        "substrate::{\n        AttachError, RendezvousId",
        "substrate::{AttachError, RendezvousId",
    ] {
        assert!(
            !src.contains(forbidden),
            "{path} must not keep old surface path residue: {forbidden}"
        );
    }
}

fn header_for(code: &[u8], mem_len: u16) -> Header {
    Header {
        code_len: code.len() as u16,
        fuel_max: 8,
        mem_len,
        hash: hibana_epf::verifier::compute_hash(code),
    }
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
        assert_no_old_surface_paths(path, &src);
        assert!(src.contains("pub fn attach_controller"));
        assert!(src.contains("pub fn attach_cluster"));
        assert!(!src.contains("pub const PROGRAM"));
        assert!(!src.contains("pub const PREFIX"));
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

    let mgmt_kinds = read_sibling_workspace_only("hibana-mgmt/src/control_kinds.rs");
    assert!(mgmt_kinds.contains("pub struct MgmtRouteKind<const LABEL: u8, const ARM: u8>;"));
    assert!(
        !mgmt_kinds.contains("pub type MgmtRoute"),
        "hibana-mgmt route kind surface must not keep per-label aliases"
    );

    let mgmt_payload = read_sibling_workspace_only("hibana-mgmt/src/payload.rs");
    assert!(mgmt_payload.contains("pub enum PolicyTarget"));
    assert!(mgmt_payload.contains("pub target: PolicyTarget"));
    assert!(
        !mgmt_payload.contains("PolicySlot"),
        "hibana-mgmt public payloads must not expose substrate advanced PolicySlot"
    );
    let _subscribe = SubscribeReq::default();
}

#[test]
fn manifest_default_lane_tracks_exact_git_revs() {
    let cargo_toml = manifest();
    assert!(cargo_toml.contains("git = \"https://github.com/hibanaworks/hibana\""));
    assert!(cargo_toml.contains("git = \"https://github.com/hibanaworks/hibana-mgmt\""));
    assert!(cargo_toml.contains("git = \"https://github.com/hibanaworks/hibana-epf\""));
    for crate_name in ["hibana", "hibana-epf", "hibana-mgmt"] {
        let _ = manifest_rev(&cargo_toml, crate_name);
    }
    assert!(!cargo_toml.contains("path = \"../hibana\""));
    assert!(!cargo_toml.contains("path = \"../hibana-mgmt\""));
    assert!(!cargo_toml.contains("path = \"../hibana-epf\""));
}

#[test]
fn lockfile_pins_resolved_git_sources() {
    if workspace_smoke_mode() {
        return;
    }
    let cargo_toml = manifest();
    let cargo_lock = lockfile();
    for crate_name in ["hibana", "hibana-epf", "hibana-mgmt"] {
        let rev = manifest_rev(&cargo_toml, crate_name);
        let expected =
            format!("source = \"git+https://github.com/hibanaworks/{crate_name}?rev={rev}#{rev}\"");
        assert!(
            cargo_lock.contains(&expected),
            "lockfile must resolve {crate_name} to the manifest rev"
        );
    }
}

#[test]
fn epf_surface_exposes_controller_lifecycle_attach_only() {
    if !workspace_smoke_mode() {
        return;
    }

    let src = read_sibling_workspace_only("hibana-epf/src/lib.rs");
    assert_no_old_surface_paths("hibana-epf/src/lib.rs", &src);
    assert!(src.contains("pub fn attach_controller"));
    assert!(!src.contains("pub fn attach_cluster"));
    assert!(!src.contains("ROLE_CLUSTER"));
    assert!(!src.contains("pub const PROGRAM"));
    assert!(!src.contains("pub const PREFIX"));
    assert!(!src.contains("pub use vm::{Slot"));
    assert!(!src.contains("const APP: g::Program<_>"));
    assert!(!src.contains("static APP: g::Program<_>"));
    assert!(!src.contains("const PROGRAM: g::Program<_>"));
    assert!(!src.contains("static PROGRAM: g::Program<_>"));
    assert!(!src.contains("project(&PROGRAM)"));
    assert!(!src.contains("project::<"));

    let kinds = read_sibling_workspace_only("hibana-epf/src/control_kinds.rs");
    assert_no_old_surface_paths("hibana-epf/src/control_kinds.rs", &kinds);
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
    assert!(!src.contains("pub struct PolicyAnnotation"));
    assert!(!src.contains("WirePayload for PolicyAnnotation"));
    assert!(!src.contains("Msg<LABEL_POLICY_LOAD, u32>"));
    assert!(!src.contains("Msg<LABEL_POLICY_ACTIVATE, u8>"));
    assert!(!src.contains("Msg<LABEL_POLICY_RESTORE, u8>"));
    assert!(!src.contains("Msg<LABEL_POLICY_ANNOTATE, PolicyAnnotation>"));

    let loader = read_sibling_workspace_only("hibana-epf/src/loader.rs");
    assert!(
        !loader.contains("pub fn commit("),
        "hibana-epf loader must not expose slot-less verification"
    );
    assert!(loader.contains("pub fn commit_for_slot"));
    assert!(loader.contains("VerifiedImage::from_parts_for_slot"));

    let verifier = read_sibling_workspace_only("hibana-epf/src/verifier.rs");
    assert!(
        !verifier.contains("pub fn new(bytes"),
        "hibana-epf verifier must not expose slot-less verification"
    );
    assert!(verifier.contains("pub fn new_for_slot"));

    let vm = read_sibling_workspace_only("hibana-epf/src/vm.rs");
    assert!(vm.contains("pub enum Slot"));
    let old_policy_slot_alias = concat!(
        "pub use hibana::substrate::policy",
        "::advanced::PolicySlot as Slot"
    );
    assert!(
        !vm.contains(old_policy_slot_alias),
        "hibana-epf must own its slot vocabulary instead of aliasing substrate advanced"
    );

    let host = read_sibling_workspace_only("hibana-epf/src/host.rs");
    assert!(
        !host.contains("pub fn into_parts("),
        "hibana-epf install failure surface must not expose duplicate decomposition"
    );
    assert!(host.contains("pub fn into_scratch(self) -> ScratchLease"));

    let _ = EPF_ROLE_CONTROLLER;
}

#[test]
fn epf_runtime_executes_under_split_repo_dependency_shape() {
    let code = [0x4B, 0x00, 0x00, 0x33, 0x00];
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
        ctx.set_policy_input([1, 0, 0, 0])
    });
    assert_eq!(action, Action::Route { arm: 1 });
}

#[test]
fn epf_runtime_rejects_non_binary_route_arm_under_split_repo_dependency_shape() {
    let code = [0x4B, 0x00, 0x00, 0x33, 0x00];
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
        ctx.set_policy_input([3, 0, 0, 0])
    });
    assert!(matches!(
        action,
        Action::Abort(info) if info.reason == ENGINE_FAIL_CLOSED && info.trap.is_none()
    ));
}
