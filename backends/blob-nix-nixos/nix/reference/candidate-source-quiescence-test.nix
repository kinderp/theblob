{ pkgs, lib, ... }:
let
  sourceRoot = lib.cleanSource ../../../..;

  harnesses = pkgs.stdenv.mkDerivation {
    pname = "blob-candidate-source-quiescence-vm";
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
      cargo build --offline --release \
        -p blob-nix-nixos-candidate-lease \
        --example hold_candidate_enqueue_lease_vm
      cargo build --offline --release \
        -p blob-nix-nixos-candidate-source-retirement \
        --example root_candidate_source_retirement_vm
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/bin"
      cp target/release/examples/root_materialization_lifecycle_vm "$out/bin/blob-lifecycle"
      cp target/release/examples/root_async_materialization_begin_vm "$out/bin/blob-queue"
      cp target/release/examples/render_trusted_candidate "$out/bin/blob-render-candidate"
      cp target/release/examples/hold_candidate_enqueue_lease_vm "$out/bin/blob-hold-lease"
      cp target/release/examples/root_candidate_source_retirement_vm "$out/bin/blob-retire-source"
      runHook postInstall
    '';
  };

  candidateSource = pkgs.writeTextDir "source-marker" ''
    BLOB_CANDIDATE_SOURCE_QUIESCENCE_OK
  '';
in
{
  name = "blob-candidate-source-quiescence";

  nodes.machine = { ... }: {
    users.users.alice = {
      isNormalUser = true;
      uid = 1000;
    };

    environment.systemPackages = [ harnesses pkgs.coreutils pkgs.nix ];

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
      "d /var/lib/theblob/materialization-lifecycle/source-retirements 0700 root root -"
      "d /nix/var/nix/gcroots/theblob-candidate-sources 0700 root root -"
      "d /nix/var/nix/gcroots/theblob-materializations 0700 root root -"
      "d /nix/var/nix/gcroots/theblob-admitted-closures 0700 root root -"
    ];

    virtualisation.writableStore = true;
    virtualisation.writableStoreUseTmpfs = false;
    virtualisation.diskSize = 4096;
    virtualisation.memorySize = 1024;
    virtualisation.cores = 2;
  };

  testScript = ''
    import re
    import shlex

    QUEUE = "${harnesses}/bin/blob-queue"
    LIFECYCLE = "${harnesses}/bin/blob-lifecycle"
    RENDER = "${harnesses}/bin/blob-render-candidate"
    HOLD = "${harnesses}/bin/blob-hold-lease"
    RETIRE = "${harnesses}/bin/blob-retire-source"
    SOURCE = "${candidateSource}"

    CANDIDATES = "/var/lib/theblob/materialization-candidates"
    PRODUCER_RECEIPTS = "/var/lib/theblob/candidate-manifest-receipts"
    JOBS = "/var/lib/theblob/materialization-begin-jobs"
    LIFECYCLE_RECEIPTS = "/var/lib/theblob/materialization-lifecycle/receipts"
    SOURCE_RETIREMENTS = "/var/lib/theblob/materialization-lifecycle/source-retirements"
    SOURCE_ROOTS = "/nix/var/nix/gcroots/theblob-candidate-sources"
    LEASE_ROOT = "/var/lib/theblob/candidate-enqueue-leases"
    FAR_FUTURE_MS = 9000000000000

    def hx(value):
        return value.encode("utf-8").hex()

    def field(output, key):
        match = re.search(r"^" + re.escape(key) + r"=(.+)$", output, re.MULTILINE)
        assert match is not None, (key, output)
        return match.group(1).strip()

    manifest_id = "manifest:quiescence-proof"
    candidate = "candidate:quiescence-proof"
    system_spec = "system:quiescence-proof"
    manifest = CANDIDATES + "/manifest-" + hx(manifest_id) + ".candidate"
    producer_receipt = PRODUCER_RECEIPTS + "/manifest-" + hx(manifest_id) + ".receipt"
    source_root = SOURCE_ROOTS + "/manifest-" + hx(manifest_id) + "-source"

    machine.start()
    machine.wait_for_unit("multi-user.target")

    # Stage one valid trusted candidate whose immutable source is a real Nix store path.
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
        "causal-id=" + hx("causal:quiescence-proof"),
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
    machine.succeed("printf %s " + shlex.quote(receipt_text) + " > " + shlex.quote(producer_receipt))
    machine.succeed("chmod 0600 " + shlex.quote(producer_receipt))
    machine.succeed("ln -s " + shlex.quote(SOURCE) + " " + shlex.quote(source_root))
    machine.succeed("test -L " + shlex.quote(source_root))

    # Create one real durable begin job, then cancel it before worker claim. This
    # gives lifecycle a terminal failed job with no native state, which is safely reclaimable.
    enqueue = machine.succeed(
        QUEUE
        + " --mode enqueue --sender :0.33 --uid 1000 --manifest-id "
        + shlex.quote(manifest_id)
    )
    request_id = field(enqueue, "request-id")
    operation = field(enqueue, "operation")
    machine.succeed(
        LIFECYCLE
        + " --mode cancel --request-id " + shlex.quote(request_id)
        + " --uid 1000 --now-ms " + str(FAR_FUTURE_MS)
    )
    machine.succeed("test -f " + JOBS + "/failed/request-" + hx(request_id) + ".job")

    # Simulate an enqueue process that acquired its pre-manifest lease and then
    # crashed before publishing a durable begin job. SIGKILL prevents Drop cleanup.
    holder_log = "/tmp/blob-holder.log"
    holder_pid = machine.succeed(
        "sh -c " + shlex.quote(
            HOLD + " --manifest-id " + shlex.quote(manifest_id)
            + " </dev/zero >" + holder_log + " 2>&1 & echo $!"
        )
    ).strip()
    machine.wait_until_succeeds("grep -q lease-active= " + holder_log)
    machine.wait_until_succeeds("test $(find " + LEASE_ROOT + "/active -type f -name '*.lease' | wc -l) -eq 1")
    machine.succeed("kill -9 " + holder_pid)
    machine.wait_until_succeeds("! kill -0 " + holder_pid + " 2>/dev/null")
    machine.succeed("test $(find " + LEASE_ROOT + "/active -type f -name '*.lease' | wc -l) -eq 1")

    # Retirement publishes its barrier, sees the abandoned active lease, and MUST
    # fail closed. Candidate selection and source retention remain intact.
    status, output = machine.execute(
        RETIRE
        + " --manifest-id " + shlex.quote(manifest_id)
        + " --now-ms " + str(FAR_FUTURE_MS)
        + " --retention-ms 0 2>&1"
    )
    assert status != 0 and "Busy" in output, (status, output)
    machine.succeed("test -f " + shlex.quote(manifest))
    machine.succeed("test -f " + shlex.quote(producer_receipt))
    machine.succeed("test -L " + shlex.quote(source_root))

    # Once the barrier is durable, every new enqueue is rejected before candidate
    # access, even though selection state still physically exists at this point.
    status, output = machine.execute(
        QUEUE
        + " --mode enqueue --sender :0.44 --uid 1000 --manifest-id "
        + shlex.quote(manifest_id)
        + " 2>&1"
    )
    assert status != 0 and "Retiring" in output, (status, output)

    # Existing daemon-startup recovery semantics are the only authority allowed to
    # clear the abandoned pre-publication lease after the old process is gone.
    recovery = machine.succeed(QUEUE + " --mode recover")
    assert field(recovery, "abandoned-enqueue-leases") == "1", recovery
    machine.succeed("test $(find " + LEASE_ROOT + "/active -type f -name '*.lease' | wc -l) -eq 0")

    # Retry resumes the already-durable barrier, proves quiescence, retires
    # selection, marks the manifest permanently retired, and removes the exact source root.
    retired = machine.succeed(
        RETIRE
        + " --manifest-id " + shlex.quote(manifest_id)
        + " --now-ms " + str(FAR_FUTURE_MS)
        + " --retention-ms 0"
    )
    assert "source-retirement=Reclaimed" in retired, retired
    machine.fail("test -e " + shlex.quote(manifest))
    machine.fail("test -e " + shlex.quote(producer_receipt))
    machine.fail("test -e " + shlex.quote(source_root))
    machine.succeed("test -f " + LIFECYCLE_RECEIPTS + "/candidate-retirement-" + hx(manifest_id) + ".receipt")
    machine.succeed("test -f " + SOURCE_RETIREMENTS + "/candidate-source-" + hx(manifest_id) + ".receipt")
    machine.succeed("test -f " + LEASE_ROOT + "/retired/" + hx(manifest_id) + ".barrier")

    # Reclaim is idempotent only with exact durable evidence; source remains absent.
    retired_again = machine.succeed(
        RETIRE
        + " --manifest-id " + shlex.quote(manifest_id)
        + " --now-ms " + str(FAR_FUTURE_MS + 1)
        + " --retention-ms 0"
    )
    assert "source-retirement=AlreadyReclaimed" in retired_again, retired_again

    # A post-retirement enqueue remains impossible and cannot recreate a job for
    # a source whose GC root has been released.
    status, output = machine.execute(
        QUEUE
        + " --mode enqueue --sender :0.55 --uid 1000 --manifest-id "
        + shlex.quote(manifest_id)
        + " 2>&1"
    )
    assert status != 0 and "Retired" in output, (status, output)
    machine.succeed("test $(find " + JOBS + "/queued -type f -name '*.job' | wc -l) -eq 0")

    # The source root is truly gone from the lifecycle graph. GC is now allowed;
    # whether the store path is collected immediately is left to Nix reachability.
    machine.succeed("${pkgs.nix}/bin/nix-store --gc >/tmp/gc.log 2>&1")
  '';
}
