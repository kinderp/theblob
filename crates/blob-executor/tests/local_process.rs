use std::env;

use blob_core::{BindingLeaseId, ImplementationId, TaskId};
use blob_executor::{
    ExecutionRequest, ExecutionStatus, LocalProcessCapsule, LocalProcessExecutor,
};

#[test]
fn local_process_capsule_returns_structured_success_result() {
    let request = ExecutionRequest {
        task: TaskId::from("task:test"),
        lease: BindingLeaseId::from("lease:test"),
        working_directory: env::temp_dir(),
        capsule: LocalProcessCapsule {
            implementation: ImplementationId::from("impl:test-process"),
            program: "sh".into(),
            args: vec!["-c".into(), "printf blob".into()],
        },
    };

    let result = LocalProcessExecutor::execute(&request).expect("process should run");

    assert_eq!(result.status, ExecutionStatus::Succeeded);
    assert_eq!(result.stdout, "blob");
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.task, request.task);
    assert_eq!(result.lease, request.lease);
}

#[test]
fn local_process_capsule_preserves_failure_as_data() {
    let request = ExecutionRequest {
        task: TaskId::from("task:test-fail"),
        lease: BindingLeaseId::from("lease:test-fail"),
        working_directory: env::temp_dir(),
        capsule: LocalProcessCapsule {
            implementation: ImplementationId::from("impl:test-process"),
            program: "sh".into(),
            args: vec!["-c".into(), "printf failure >&2; exit 7".into()],
        },
    };

    let result = LocalProcessExecutor::execute(&request).expect("process should run");

    assert_eq!(result.status, ExecutionStatus::Failed);
    assert_eq!(result.exit_code, Some(7));
    assert_eq!(result.stderr, "failure");
}
