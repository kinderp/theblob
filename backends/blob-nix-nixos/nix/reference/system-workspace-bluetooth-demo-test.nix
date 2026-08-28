{ pkgs, lib, ... }:
let
  sourceRoot = lib.cleanSource ../../../..;
  trustedBaseModule = pkgs.writeText "blob-system-workspace-demo-base.nix" (builtins.readFile ./base.nix);

  harnesses = pkgs.stdenv.mkDerivation {
    pname = "blob-system-workspace-bluetooth-demo-vm";
    version = "0.1.0";
    src = sourceRoot;
    nativeBuildInputs = [ pkgs.cargo pkgs.rustc ];
    buildPhase = ''
      runHook preBuild
      export HOME="$TMPDIR/home"
      export CARGO_HOME="$TMPDIR/cargo-home"
      mkdir -p "$HOME" "$CARGO_HOME"
      cargo build --offline --release \
        -p blob-nix-nixos-system-workspace \
        --example render_bluetooth_demo_system_spec
      cargo build --offline --release \
        -p blob-nix-nixos-candidate-producer \
        --example root_systemspec_candidate_producer_vm
      cargo build --offline --release \
        -p blob-nix-nixos-materialization-begin-queue \
        --example root_async_materialization_begin_vm
      cargo build --offline --release \
        -p blob-nix-nixos-materialization-authority \
        --example root_materialization_authority_vm
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/bin"
      cp target/release/examples/render_bluetooth_demo_system_spec \
        "$out/bin/blob-render-bluetooth-demo-system-spec"
      cp target/release/examples/root_systemspec_candidate_producer_vm \
        "$out/bin/blob-systemspec-candidate-producer-vm"
      cp target/release/examples/root_async_materialization_begin_vm \
        "$out/bin/blob-async-materialization-begin-vm"
      cp target/release/examples/root_materialization_authority_vm \
        "$out/bin/blob-materialization-authority-vm"
      runHook postInstall
    '';
  };
in
{
  name = "blob-system-workspace-bluetooth-demo";

  nodes.machine = { ... }: {
    nix.settings.experimental-features = [ "nix-command" "flakes" ];
    nix.settings.substituters = lib.mkForce [ ];
    system.extraDependencies = [ pkgs.path trustedBaseModule ];

    users.users.alice = {
      isNormalUser = true;
      uid = 1000;
    };

    environment.systemPackages = [ pkgs.nix harnesses ];

    systemd.tmpfiles.rules = [
      "d /var/lib/theblob 0700 root root -"
      "d /var/lib/theblob/materialization-candidates 0700 root root -"
      "d /var/lib/theblob/candidate-manifest-receipts 0700 root root -"
      "d /var/lib/theblob/candidate-source-staging 0700 root root -"
      "d /var/lib/theblob/materialization-intents 0700 root root -"
      "d /var/lib/theblob/materialization-intents/pending 0700 root root -"
      "d /var/lib/theblob/materialization-intents/completed 0700 root root -"
      "d /var/lib/theblob/materialization-admissions 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs/queued 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs/running 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs/completed 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs/failed 0700 root root -"
      "d /nix/var/nix/gcroots/theblob-candidate-sources 0700 root root -"
      "d /nix/var/nix/gcroots/theblob-materializations 0700 root root -"
    ];

    virtualisation.writableStore = true;
    virtualisation.writableStoreUseTmpfs = false;
    virtualisation.diskSize = 12288;
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;
  };

  testScript = ''
    import re
    import shlex

    RENDER = "${harnesses}/bin/blob-render-bluetooth-demo-system-spec"
    PRODUCER = "${harnesses}/bin/blob-systemspec-candidate-producer-vm"
    BEGIN = "${harnesses}/bin/blob-async-materialization-begin-vm"
    AUTHORITY = "${harnesses}/bin/blob-materialization-authority-vm"
    NIX = "${pkgs.nix}/bin/nix"
    NIX_STORE = "${pkgs.nix}/bin/nix-store"
    NIXPKGS = "${pkgs.path}"
    BASE = "${trustedBaseModule}"
    STAGING = "/var/lib/theblob/candidate-source-staging"

    def field(output, key):
        match = re.search(r"^" + re.escape(key) + r"=(.+)$", output, re.MULTILINE)
        assert match is not None, (key, output)
        return match.group(1).strip()

    machine.start()
    machine.wait_for_unit("multi-user.target")

    # Product side: render the canonical request from the backend-neutral
    # System Workspace proposal. The unprivileged caller emits no Nix/native field.
    spec = machine.succeed("su -s /bin/sh alice -c " + shlex.quote(RENDER))
    assert spec.startswith("theblob-system-spec-v1\n"), spec
    assert spec.endswith("\n"), spec
    assert "hardware.bluetooth.enable" not in spec, spec
    assert "raw-nix" not in spec, spec
    assert "--impure" not in spec, spec

    # Trusted root producer derives native Nix and every candidate identity.
    produce = (
        "printf %s " + shlex.quote(spec)
        + " | " + PRODUCER
        + " --sender :0.41"
        + " --nix " + shlex.quote(NIX)
        + " --nixpkgs-source " + shlex.quote(NIXPKGS)
        + " --base-module " + shlex.quote(BASE)
        + " --staging-root " + shlex.quote(STAGING)
    )
    evidence = machine.succeed(produce)
    manifest_id = field(evidence, "manifest-id")
    candidate = field(evidence, "candidate")
    system_spec = field(evidence, "system-spec")
    source = field(evidence, "source")
    source_gcroot = field(evidence, "source-gcroot")

    assert system_spec == "system:demo-workspace:proposal", evidence
    assert source.startswith("/nix/store/"), evidence
    assert "translation=feature:bluetooth=enabled -> hardware.bluetooth.enable" in evidence, evidence
    machine.succeed("grep -Fxq '  hardware.bluetooth.enable = true;' " + shlex.quote(source + "/generated.nix"))
    machine.fail("grep -Fxq '  hardware.bluetooth.enable = false;' " + shlex.quote(source + "/generated.nix"))
    machine.succeed("test -L " + shlex.quote(source_gcroot))
    assert machine.succeed("readlink " + shlex.quote(source_gcroot)).strip() == source

    # Enter the existing durable async begin boundary. The product request still
    # selects only the trusted manifest id; it cannot replace source/attribute.
    queued = machine.succeed(
        BEGIN
        + " --mode enqueue"
        + " --sender :0.42"
        + " --uid 1000"
        + " --manifest-id " + shlex.quote(manifest_id)
    )
    request_id = field(queued, "request-id")
    operation = field(queued, "operation")
    assert request_id.startswith("begin-request:"), queued
    assert operation.startswith("op:materialize-"), queued

    worked = machine.succeed(
        BEGIN
        + " --mode work-one"
        + " --nix " + shlex.quote(NIX)
        + " --nix-store " + shlex.quote(NIX_STORE)
    )
    assert field(worked, "request-id") == request_id, worked
    assert field(worked, "operation") == operation, worked
    assert "state=completed" in worked, worked
    assert field(worked, "candidate") == candidate, worked
    assert field(worked, "system-spec") == system_spec, worked
    assert field(worked, "source") == source, worked
    build_target = field(worked, "build-target")
    expected_output = field(worked, "expected-output")

    # Materialization itself remains non-root. Root completion verifies the exact
    # realized output; it does not silently become the builder.
    machine.succeed(
        "su -s /bin/sh alice -c "
        + shlex.quote(NIX + " build --no-link " + build_target)
    )
    machine.succeed("test -e " + shlex.quote(expected_output))

    completed = machine.succeed(
        AUTHORITY
        + " --mode complete"
        + " --operation " + shlex.quote(operation)
        + " --nix " + shlex.quote(NIX)
        + " --nix-store " + shlex.quote(NIX_STORE)
    )
    assert field(completed, "operation") == operation, completed
    assert field(completed, "closure") == expected_output, completed
    assert "provenance=exact-derivation-output-match" in completed, completed

    # The UI-visible semantic request therefore reached an admitted immutable
    # NixOS closure without any persistent/live activation step.
    machine.succeed("test -d /var/lib/theblob/materialization-admissions")
    machine.fail("test -e /run/current-system/specialisation/blob-demo")
  '';
}
