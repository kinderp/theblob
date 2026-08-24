use blob_core::{BindingLeaseId, ImplementationId, TaskId};
use blob_wasm_wasmtime::{
    ComponentExecutionRequest, DenyByDefaultComponentRuntime, RuntimeStage, WasmComponentCapsule,
};

const PURE_COMPONENT: &str = r#"
(component
    (core module $m
        (func (export "run") (result i32)
            i32.const 0)
    )
    (core instance $i (instantiate $m))
    (func (export "run") (result u32)
        (canon lift (core func $i "run")))
)
"#;

const COMPONENT_REQUIRING_HOST_IMPORT: &str = r#"
(component
    (import "forbidden-host-action" (func $forbidden))
)
"#;

fn request(component: &str, entrypoint: &str) -> ComponentExecutionRequest {
    ComponentExecutionRequest {
        task: TaskId::from("task:wasm"),
        lease: BindingLeaseId::from("lease:wasm"),
        capsule: WasmComponentCapsule {
            implementation: ImplementationId::from("impl:wasm-component"),
            component_bytes: component.as_bytes().to_vec(),
            entrypoint: entrypoint.into(),
        },
    }
}

#[test]
fn pure_component_executes_without_any_host_capabilities() {
    let runtime = DenyByDefaultComponentRuntime::new();
    let result = runtime
        .execute_u32(&request(PURE_COMPONENT, "run"))
        .expect("pure component should execute with an empty linker");

    assert_eq!(result.return_code, 0);
    assert_eq!(result.task, TaskId::from("task:wasm"));
    assert_eq!(result.lease, BindingLeaseId::from("lease:wasm"));
    assert!(result.runtime.starts_with("wasmtime-component@"));
}

#[test]
fn undeclared_host_import_is_denied_at_instantiation() {
    let runtime = DenyByDefaultComponentRuntime::new();
    let error = runtime
        .execute_u32(&request(COMPONENT_REQUIRING_HOST_IMPORT, "run"))
        .expect_err("empty linker must not satisfy an undeclared host import");

    assert_eq!(error.stage, RuntimeStage::Instantiate);
    assert!(error.message.contains("forbidden-host-action"));
}

#[test]
fn wrong_export_is_reported_without_mutating_authority() {
    let runtime = DenyByDefaultComponentRuntime::new();
    let error = runtime
        .execute_u32(&request(PURE_COMPONENT, "missing"))
        .expect_err("missing export must be a structured runtime error");

    assert_eq!(error.stage, RuntimeStage::ExportLookup);
}
