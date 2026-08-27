{ pkgs, lib, ... }:
let
  sourceRoot = lib.cleanSource ../../../..;

  lifecycleHarnesses = pkgs.stdenv.mkDerivation {
    pname = "blob-materialization-lifecycle-vm";
    version = "0.1.0";
    src = sourceRoot;
    nativeBuildInputs = [ pkgs.cargo pkgs.rustc ];
    buildPhase = ''
      runHook preBuild
      export HOME="$TMPDIR/home"
      export CARGO_HOME="$TMPDIR/cargo-home"
      mkdir -p "$HOME" "$CARGO_HOME"
      cargo build --offline --release \
        -p blob-nix-nixos-materialization-lifecycle \
        --example root_materialization_lifecycle_vm
      cargo build --offline --release \
        -p blob-nix-nixos-materialization-begin-queue \
        --example root_async_materialization_begin_vm
      cargo build --offline --release \
        -p blob-nix-nixos-materialization-begin \
        --example render_trusted_candidate
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/bin"
      cp target/release/examples/root_materialization_lifecycle_vm \
        "$out/bin/blob-materialization-lifecycle-vm"
      cp target/release/examples/root_async_materialization_begin_vm \
        "$out/bin/blob-async-materialization-begin-vm"
      cp target/release/examples/render_trusted_candidate \
        "$out/bin/blob-render-trusted-candidate"
      runHook postInstall
    '';
  };

  # A tiny hermetic materialization oracle keeps this checkpoint focused on the
  # lifecycle/GC contract instead of rebuilding a full NixOS closure. ADR-0038
  # separately proves SystemSpec -> trusted candidate production.
  materializationFlake = pkgs.writeTextDir "flake.nix" ''
    {
      inputs.builder = {
        url = "path:${pkgs.pkgsStatic.busybox}";
        flake = false;
      };

      outputs = { self, builder }: {
        packages.x86_64-linux.candidate = builtins.derivation {
          name = "blob-materialization-lifecycle-candidate";
          system = "x86_64-linux";
          builder = "''${builder.outPath}/bin/busybox";
          args = [
            "sh"
            "-c"
            "\"''${builder.outPath}/bin/busybox\" mkdir -p \"$out\"; printf '%s\\n' BLOB_MATERIALIZATION_LIFECYCLE_OK > \"$out/blob-marker\""
          ];
        };
      };
    }
  '';
in
{
  name = "blob-materialization-lifecycle";

  nodes.machine = { ... }: {
    nix.settings.experimental-features = [ "nix-command" "flakes" ];
    nix.settings.substituters = lib.mkForce [ ];
    system.extraDependencies = [ materializationFlake pkgs.pkgsStatic.busybox ];

    users.users.alice = {
      isNormalUser = true;
      uid = 1000;
    };
    users.users.bob = {
      isNormalUser = true;
      uid = 1001;
    };

    environment.systemPackages = [ lifecycleHarnesses pkgs.nix ];

    systemd.tmpfiles.rules = [
      "d /var/lib/theblob 0700 root root -"
      "d /var/lib/theblob/materialization-candidates 0700 root root -"
      "d /var/lib/theblob/candidate-manifest-receipts 0700 root root -"
      "d /var/lib/theblob/materialization-intents 0700 root root -"
      "d /var/lib/theblob/materialization-intents/pending 0700 root root -"
      "d /var/lib/theblob/materialization-intents/completed 0700 root root -"
      "d /var/lib/theblob/materialization-admissions 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs/queued 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs/running 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs/completed 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs/failed 0700 root root -"
      "d /var/lib/theblob/materialization-lifecycle 0700 root root -"
      "d /var/lib/theblob/materialization-lifecycle/receipts 0700 root root -"
      "d /nix/var/nix/gcroots/theblob-candidate-sources 0700 root root -"
      "d /nix/var/nix/gcroots/theblob-materializations 0700 root root -"
      "d /nix/var/nix/gcroots/theblob-admitted-closures 0700 root root -"
    ];

    virtualisation.writableStore = true;
    virtualisation.writableStoreUseTmpfs = false;
    virtualisation.diskSize = 6144;
    virtualisation.memorySize = 1536;
    virtualisation.cores = 2;
  };

  testScript = ''
    import re
    import shlex

    LIFECYCLE = "${lifecycleHarnesses}/bin/blob-materialization-lifecycle-vm"
    QUEUE = "${lifecycleHarnesses}/bin/blob-async-materialization-begin-vm"
    RENDER = "${lifecycleHarnesses}/bin/blob-render-trusted-candidate"
    SOURCE = "${materializationFlake}"
    NIX = "${pkgs.nix}/bin/nix"
    NIX_STORE = "${pkgs.nix}/bin/nix-store"

    CANDIDATES = "/var/lib/theblob/materialization-candidates"
    PRODUCER_RECEIPTS = "/var/lib/theblob/candidate-manifest-receipts"
    JOBS = "/var/lib/theblob/materialization-begin-jobs"
    INTENTS = "/var/lib/theblob/materialization-intents"
    ADMISSIONS = "/var/lib/theblob/materialization-admissions"
    LIFECYCLE_RECEIPTS = "/var/lib/theblob/materialization-lifecycle/receipts"
    SOURCE_ROOTS = "/nix/var/nix/gcroots/theblob-candidate-sources"
    DERIVATION_ROOTS = "/nix/var/nix/gcroots/theblob-materializations"
    CLOSURE_ROOTS = "/nix/var/nix/gcroots/theblob-admitted-closures"
    FAR_FUTURE_MS = 9000000000000

    def hx(value):
        return value.encode("utf-8").hex()

    def field(output, key):
        match = re.search(r"^" + re.escape(key) + r"=(.+)$", output, re.MULTILINE)
        assert match is not None, (key, output)
        return match.group(1).strip()

    def manifest_path(manifest_id):
        return CANDIDATES + "/manifest-" + hx(manifest_id) + ".candidate"

    def producer_receipt_path(manifest_id):
        return PRODUCER_RECEIPTS + "/manifest-" + hx(manifest_id) + ".receipt"

    def source_gcroot(manifest_id):
        return SOURCE_ROOTS + "/manifest-" + hx(manifest_id) + "-source"

    def job_path(state, request_id):
        return JOBS + "/" + state + "/request-" + hx(request_id) + ".job"

    def derivation_gcroot(operation):
        return DERIVATION_ROOTS + "/operation-" + hx(operation) + "-derivation"

    def closure_gcroot(operation):
        return CLOSURE_ROOTS + "/operation-" + hx(operation) + "-closure"

    def lifecycle_receipt(kind, subject):
        return LIFECYCLE_RECEIPTS + "/" + kind + "-" + hx(subject) + ".receipt"

    def stage_candidate(suffix):
        manifest_id = "manifest:lifecycle-" + suffix
        candidate = "candidate:lifecycle-" + suffix
        system_spec = "system:lifecycle-" + suffix
        manifest = manifest_path(manifest_id)
        receipt = producer_receipt_path(manifest_id)
        source_root = source_gcroot(manifest_id)

        render = (
            RENDER
            + " --manifest-id " + shlex.quote(manifest_id)
            + " --candidate " + shlex.quote(candidate)
            + " --system-spec " + shlex.quote(system_spec)
            + " --source " + shlex.quote(SOURCE)
            + " --attribute packages.x86_64-linux.candidate"
        )
        machine.succeed(render + " > " + shlex.quote(manifest))
        machine.succeed("chmod 0600 " + shlex.quote(manifest))

        receipt_text = "\n".join([
            "theblob-candidate-manifest-receipt-v1",
            "causal-id=" + hx("causal:lifecycle-" + suffix),
            "occurred-at-unix-ms=1",
            "requester-system-bus=" + hx(":0.33"),
            "manifest-id=" + hx(manifest_id),
            "candidate=" + hx(candidate),
            "system-spec=" + hx(system_spec),
            "immutable-flake-root=" + hx(SOURCE),
            "canonical-system-spec=",
            "translation-evidence-count=0",
            "",
        ])
        quoted = shlex.quote(receipt_text)
        machine.succeed("printf %s " + quoted + " > " + shlex.quote(receipt))
        machine.succeed("chmod 0600 " + shlex.quote(receipt))
        machine.succeed("ln -s " + shlex.quote(SOURCE) + " " + shlex.quote(source_root))

        machine.succeed("test \"$(stat -c '%u:%a' " + shlex.quote(manifest) + ")\" = 0:600")
        machine.succeed("test \"$(stat -c '%u:%a' " + shlex.quote(receipt) + ")\" = 0:600")
        machine.succeed("test \"$(readlink " + shlex.quote(source_root) + ")\" = " + shlex.quote(SOURCE))
        return manifest_id

    def enqueue(manifest_id, uid=1000):
        status, output = machine.execute(
            QUEUE
            + " --mode enqueue"
            + " --sender :0.33"
            + " --uid " + str(uid)
            + " --manifest-id " + shlex.quote(manifest_id)
            + " 2>&1"
        )
        assert status == 0, (status, output)
        return field(output, "request-id"), field(output, "operation")

    def work_one():
        status, output = machine.execute(
            QUEUE
            + " --mode work-one"
            + " --nix " + shlex.quote(NIX)
            + " --nix-store " + shlex.quote(NIX_STORE)
            + " 2>&1"
        )
        assert status == 0, (status, output)
        assert "state=completed" in output, output
        return output

    def lifecycle(mode, *pairs):
        command = LIFECYCLE + " --mode " + mode
        for key, value in pairs:
            command += " --" + key + " " + shlex.quote(str(value))
        return machine.execute(command + " 2>&1")

    machine.start()
    machine.wait_for_unit("multi-user.target")

    # --- Main retention handoff: begin -> non-root build -> closure root -> admission.
    main_manifest = stage_candidate("main")
    main_request, main_operation = enqueue(main_manifest)
    work_output = work_one()
    assert field(work_output, "request-id") == main_request, work_output
    assert field(work_output, "operation") == main_operation, work_output
    derivation = field(work_output, "derivation")
    expected = field(work_output, "expected-output")
    build_target = field(work_output, "build-target")
    assert build_target == derivation + "^out", (build_target, derivation)
    machine.succeed("test -L " + shlex.quote(derivation_gcroot(main_operation)))
    machine.succeed("test -e " + shlex.quote(job_path("completed", main_request)))
    machine.succeed("test -n \"$(find " + INTENTS + "/pending -maxdepth 1 -type f -name '*.intent' -print -quit)\"")

    # A begin-completed queue job is not enough to retire the candidate.
    code, output = lifecycle(
        "retire-candidate",
        ("manifest-id", main_manifest),
        ("now-ms", FAR_FUTURE_MS),
        ("retention-ms", 0),
    )
    assert code != 0, (code, output)
    machine.succeed("test -e " + shlex.quote(manifest_path(main_manifest)))

    # Safe finalization is verification-only and cannot realize a missing output.
    code, output = lifecycle(
        "finalize",
        ("operation", main_operation),
        ("nix", NIX),
        ("nix-store", NIX_STORE),
    )
    assert code != 0, (code, output)
    machine.succeed("test ! -e " + shlex.quote(expected))
    machine.succeed("test ! -e " + shlex.quote(closure_gcroot(main_operation)))

    alice_build = (
        "HOME=/home/alice " + NIX
        + " build --no-link --no-write-lock-file " + shlex.quote(build_target)
    )
    machine.succeed("su -s /bin/sh alice -c " + shlex.quote(alice_build))
    machine.succeed("grep -qx BLOB_MATERIALIZATION_LIFECYCLE_OK " + shlex.quote(expected + "/blob-marker"))

    code, finalize_output = lifecycle(
        "finalize",
        ("operation", main_operation),
        ("nix", NIX),
        ("nix-store", NIX_STORE),
    )
    assert code == 0, (code, finalize_output)
    assert field(finalize_output, "closure") == expected, finalize_output
    assert field(finalize_output, "closure-gcroot") == closure_gcroot(main_operation), finalize_output
    machine.succeed("test \"$(readlink " + shlex.quote(closure_gcroot(main_operation)) + ")\" = " + shlex.quote(expected))
    machine.succeed("test ! -e " + shlex.quote(derivation_gcroot(main_operation)))
    machine.succeed("test -z \"$(find " + INTENTS + "/pending -maxdepth 1 -type f -name '*.intent' -print -quit)\"")
    completed_intent = machine.succeed(
        "find " + INTENTS + "/completed -maxdepth 1 -type f -name '*.intent' -print -quit"
    ).strip()
    admission = machine.succeed(
        "find " + ADMISSIONS + " -maxdepth 1 -type f -name '*.admission' -print -quit"
    ).strip()
    assert completed_intent and admission, (completed_intent, admission)
    machine.succeed("grep -q " + shlex.quote("system-closure=" + hx(expected)) + " " + shlex.quote(admission))

    # Reconciliation may remove an exact leftover derivation root only because
    # completed intent + admission + admitted closure root all agree.
    machine.succeed("ln -s " + shlex.quote(derivation) + " " + shlex.quote(derivation_gcroot(main_operation)))
    code, output = lifecycle(
        "reconcile-derivation",
        ("operation", main_operation),
        ("now-ms", FAR_FUTURE_MS),
    )
    assert code == 0 and "ReleasedAfterAdmission" in output, (code, output)
    machine.succeed("test ! -e " + shlex.quote(derivation_gcroot(main_operation)))

    # Unknown orphan roots are retained, never guessed away.
    orphan_operation = "op:materialize-orphan-lifecycle"
    orphan_root = derivation_gcroot(orphan_operation)
    machine.succeed("ln -s " + shlex.quote(derivation) + " " + shlex.quote(orphan_root))
    code, output = lifecycle(
        "reconcile-derivation",
        ("operation", orphan_operation),
        ("now-ms", FAR_FUTURE_MS),
    )
    assert code == 0 and "OrphanRetained" in output, (code, output)
    machine.succeed("test -L " + shlex.quote(orphan_root))
    machine.succeed("rm " + shlex.quote(orphan_root))

    # Real Nix GC cannot collect the admitted output after derivation retention
    # has been handed off to the exact admitted-closure root.
    machine.succeed(NIX_STORE + " --gc")
    machine.succeed("grep -qx BLOB_MATERIALIZATION_LIFECYCLE_OK " + shlex.quote(expected + "/blob-marker"))
    machine.succeed("test \"$(readlink " + shlex.quote(closure_gcroot(main_operation)) + ")\" = " + shlex.quote(expected))

    # Terminal job retirement is blocked until candidate selection retirement is durable.
    code, output = lifecycle(
        "retire-job",
        ("request-id", main_request),
        ("now-ms", FAR_FUTURE_MS),
        ("retention-ms", 0),
    )
    assert code != 0, (code, output)
    machine.succeed("test -e " + shlex.quote(job_path("completed", main_request)))

    # Candidate selection retirement removes manifest + producer receipt but keeps
    # the source GC root until a later shared enqueue/quiescence protocol exists.
    code, output = lifecycle(
        "retire-candidate",
        ("manifest-id", main_manifest),
        ("now-ms", FAR_FUTURE_MS),
        ("retention-ms", 0),
    )
    assert code == 0, (code, output)
    machine.succeed("test ! -e " + shlex.quote(manifest_path(main_manifest)))
    machine.succeed("test ! -e " + shlex.quote(producer_receipt_path(main_manifest)))
    machine.succeed("test -L " + shlex.quote(source_gcroot(main_manifest)))
    machine.succeed("test \"$(readlink " + shlex.quote(source_gcroot(main_manifest)) + ")\" = " + shlex.quote(SOURCE))
    retirement_receipt = lifecycle_receipt("candidate-retirement", main_manifest)
    machine.succeed("test \"$(stat -c '%u:%a' " + shlex.quote(retirement_receipt) + ")\" = 0:600")
    machine.succeed("test -L " + shlex.quote(closure_gcroot(main_operation)))

    code, output = lifecycle(
        "retire-job",
        ("request-id", main_request),
        ("now-ms", FAR_FUTURE_MS),
        ("retention-ms", 0),
    )
    assert code == 0, (code, output)
    machine.succeed("test ! -e " + shlex.quote(job_path("completed", main_request)))
    machine.succeed("test -e " + shlex.quote(admission))
    machine.succeed("test -e " + shlex.quote(completed_intent))
    machine.succeed("test -L " + shlex.quote(closure_gcroot(main_operation)))

    # --- Queue lifecycle: stale queued work with no native state can expire.
    expiry_manifest = stage_candidate("expiry")
    expiry_request, expiry_operation = enqueue(expiry_manifest)
    code, output = lifecycle(
        "expire",
        ("request-id", expiry_request),
        ("now-ms", FAR_FUTURE_MS),
        ("retention-ms", 0),
    )
    assert code == 0, (code, output)
    machine.succeed("test ! -e " + shlex.quote(job_path("queued", expiry_request)))
    machine.succeed("test -e " + shlex.quote(job_path("failed", expiry_request)))
    machine.succeed("test \"$(stat -c '%u:%a' " + shlex.quote(lifecycle_receipt("queued-expiry", expiry_request)) + ")\" = 0:600")

    # A queued job with any operation GC-root is treated as native durable state
    # and cannot be expired, even before an intent exists.
    rooted_manifest = stage_candidate("rooted-queue")
    rooted_request, rooted_operation = enqueue(rooted_manifest)
    rooted_gc = derivation_gcroot(rooted_operation)
    machine.succeed("ln -s " + shlex.quote(derivation) + " " + shlex.quote(rooted_gc))
    code, output = lifecycle(
        "expire",
        ("request-id", rooted_request),
        ("now-ms", FAR_FUTURE_MS),
        ("retention-ms", 0),
    )
    assert code != 0, (code, output)
    machine.succeed("test -e " + shlex.quote(job_path("queued", rooted_request)))
    machine.succeed("test -L " + shlex.quote(rooted_gc))
    machine.succeed("rm " + shlex.quote(rooted_gc))

    # Explicit queued cancellation is UID-bound.
    cancel_manifest = stage_candidate("cancel")
    cancel_request, cancel_operation = enqueue(cancel_manifest)
    code, output = lifecycle(
        "cancel",
        ("request-id", cancel_request),
        ("uid", 1001),
        ("now-ms", FAR_FUTURE_MS),
    )
    assert code != 0, (code, output)
    machine.succeed("test -e " + shlex.quote(job_path("queued", cancel_request)))
    code, output = lifecycle(
        "cancel",
        ("request-id", cancel_request),
        ("uid", 1000),
        ("now-ms", FAR_FUTURE_MS),
    )
    assert code == 0, (code, output)
    machine.succeed("test -e " + shlex.quote(job_path("failed", cancel_request)))

    # Running work is never cancelled by this lifecycle API. The direct rename is
    # a root-owned KVM fixture for state classification; #33 separately proves the
    # real queue's atomic queued -> running claim.
    running_manifest = stage_candidate("running")
    running_request, running_operation = enqueue(running_manifest)
    machine.succeed(
        "mv " + shlex.quote(job_path("queued", running_request)) + " " + shlex.quote(job_path("running", running_request))
    )
    code, output = lifecycle(
        "cancel",
        ("request-id", running_request),
        ("uid", 1000),
        ("now-ms", FAR_FUTURE_MS),
    )
    assert code != 0, (code, output)
    machine.succeed("test -e " + shlex.quote(job_path("running", running_request)))
    machine.succeed(
        "mv " + shlex.quote(job_path("running", running_request)) + " " + shlex.quote(job_path("queued", running_request))
    )

    # Layout mode conflicts fail closed before mutation.
    mode_manifest = stage_candidate("mode-conflict")
    mode_request, mode_operation = enqueue(mode_manifest)
    machine.succeed("chmod 0755 " + LIFECYCLE_RECEIPTS)
    code, output = lifecycle(
        "cancel",
        ("request-id", mode_request),
        ("uid", 1000),
        ("now-ms", FAR_FUTURE_MS),
    )
    assert code != 0, (code, output)
    machine.succeed("test -e " + shlex.quote(job_path("queued", mode_request)))
    machine.succeed("chmod 0700 " + LIFECYCLE_RECEIPTS)

    # A symlink at the deterministic decision-receipt path is not followed.
    symlink_receipt = lifecycle_receipt("queued-cancel", mode_request)
    machine.succeed("ln -s /tmp " + shlex.quote(symlink_receipt))
    code, output = lifecycle(
        "cancel",
        ("request-id", mode_request),
        ("uid", 1000),
        ("now-ms", FAR_FUTURE_MS),
    )
    assert code != 0, (code, output)
    machine.succeed("test -e " + shlex.quote(job_path("queued", mode_request)))
    machine.succeed("rm " + shlex.quote(symlink_receipt))
    code, output = lifecycle(
        "cancel",
        ("request-id", mode_request),
        ("uid", 1000),
        ("now-ms", FAR_FUTURE_MS),
    )
    assert code == 0, (code, output)

    # No activation action is performed in this checkpoint; the admitted output
    # remains merely retained for the later request/activation lifecycle.
    machine.succeed("test -L " + shlex.quote(closure_gcroot(main_operation)))
    machine.succeed("grep -qx BLOB_MATERIALIZATION_LIFECYCLE_OK " + shlex.quote(expected + "/blob-marker"))
  '';
}
