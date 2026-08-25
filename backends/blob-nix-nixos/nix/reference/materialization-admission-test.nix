{ pkgs, lib, ... }:
let
  sourceRoot = lib.cleanSource ../../../..;

  authorityHarness = pkgs.stdenv.mkDerivation {
    pname = "blob-materialization-authority-vm";
    version = "0.1.0";
    src = sourceRoot;
    nativeBuildInputs = [ pkgs.cargo pkgs.rustc ];
    buildPhase = ''
      runHook preBuild
      export HOME="$TMPDIR/home"
      export CARGO_HOME="$TMPDIR/cargo-home"
      mkdir -p "$HOME" "$CARGO_HOME"
      cargo build --offline --release \
        -p blob-nix-nixos-materialization-authority \
        --example root_materialization_authority_vm
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/bin"
      cp target/release/examples/root_materialization_authority_vm \
        "$out/bin/blob-materialization-authority-vm"
      runHook postInstall
    '';
  };

  # Deliberately avoid nixpkgs/stdenv inside the guest materialization. The
  # derivations use only BusyBox, which is already copied into the VM store, so
  # Alice's build proves local non-root realization without network substitutes.
  materializationFlake = pkgs.writeTextDir "flake.nix" ''
    {
      outputs = { self }: {
        packages.x86_64-linux.candidate = builtins.derivation {
          name = "blob-materialization-admission-candidate";
          system = "x86_64-linux";
          builder = "${pkgs.busybox}/bin/sh";
          args = [ "-c" "mkdir -p \"$out\"; printf '%s\\n' BLOB_MATERIALIZATION_ADMISSION_OK > \"$out/blob-marker\"" ];
        };
        packages.x86_64-linux.decoy = builtins.derivation {
          name = "blob-materialization-admission-decoy";
          system = "x86_64-linux";
          builder = "${pkgs.busybox}/bin/sh";
          args = [ "-c" "mkdir -p \"$out\"; printf '%s\\n' DECOY > \"$out/blob-marker\"" ];
        };
      };
    }
  '';

  unsafeSource = pkgs.runCommand "blob-materialization-symlink-source" { } ''
    mkdir -p "$out"
    ln -s /tmp "$out/ref"
  '';
in
{
  name = "blob-materialization-admission";

  nodes.machine = { ... }: {
    nix.settings.experimental-features = [ "nix-command" "flakes" ];
    nix.settings.substituters = lib.mkForce [ ];
    users.users.alice = {
      isNormalUser = true;
      uid = 1000;
    };
    environment.systemPackages = [
      authorityHarness
      pkgs.nix
      pkgs.busybox
    ];
    systemd.tmpfiles.rules = [
      "d /var/lib/theblob 0700 root root -"
      "d /var/lib/theblob/materialization-intents 0700 root root -"
      "d /var/lib/theblob/materialization-intents/pending 0700 root root -"
      "d /var/lib/theblob/materialization-intents/completed 0700 root root -"
      "d /var/lib/theblob/materialization-admissions 0700 root root -"
    ];
    virtualisation.memorySize = 1536;
    virtualisation.cores = 2;
  };

  testScript = ''
    import re
    import shlex

    HARNESS = "${authorityHarness}/bin/blob-materialization-authority-vm"
    SOURCE = "${materializationFlake}"
    UNSAFE_SOURCE = "${unsafeSource}/ref"
    NIX = "${pkgs.nix}/bin/nix"
    NIX_STORE = "${pkgs.nix}/bin/nix-store"
    OP = "op:blob-materialization-admission-vm"
    CANDIDATE = "candidate:blob-materialization-admission-vm"
    SYSTEM_SPEC = "system:blob-materialization-admission-vm"

    def root_begin(operation=OP, attribute="packages.x86_64-linux.candidate", source=SOURCE):
        return machine.execute(
            HARNESS
            + " --mode begin"
            + " --operation " + shlex.quote(operation)
            + " --candidate " + shlex.quote(CANDIDATE)
            + " --system-spec " + shlex.quote(SYSTEM_SPEC)
            + " --source " + shlex.quote(source)
            + " --attribute " + shlex.quote(attribute)
            + " --nix " + shlex.quote(NIX)
            + " --nix-store " + shlex.quote(NIX_STORE)
            + " 2>&1"
        )

    def root_complete(operation=OP):
        return machine.execute(
            HARNESS
            + " --mode complete"
            + " --operation " + shlex.quote(operation)
            + " --nix " + shlex.quote(NIX)
            + " --nix-store " + shlex.quote(NIX_STORE)
            + " 2>&1"
        )

    def field(output, key):
        match = re.search(r"^" + re.escape(key) + r"=(.+)$", output, re.MULTILINE)
        assert match is not None, (key, output)
        return match.group(1).strip()

    machine.start()
    machine.wait_for_unit("multi-user.target")

    # A lexically store-local path that resolves through a symlink to /tmp is not
    # an immutable source and must be rejected before Nix evaluation.
    status, unsafe_output = root_begin(
        operation="op:blob-materialization-symlink-source",
        source=UNSAFE_SOURCE,
    )
    assert status != 0, (status, unsafe_output)
    assert "InvalidImmutableFlakeRoot" in unsafe_output or "immutable" in unsafe_output.lower(), unsafe_output
    machine.succeed("test -z \"$(find /var/lib/theblob/materialization-intents/pending -maxdepth 1 -type f -print -quit)\"")

    status, begin_output = root_begin()
    assert status == 0, (status, begin_output)
    derivation = field(begin_output, "derivation")
    expected = field(begin_output, "expected-output")
    target = field(begin_output, "build-target")

    assert derivation.startswith("/nix/store/") and derivation.endswith(".drv"), derivation
    assert expected.startswith("/nix/store/"), expected
    assert target == derivation + "^out", (target, derivation)

    intent = machine.succeed(
        "find /var/lib/theblob/materialization-intents/pending -maxdepth 1 -type f -name '*.intent' -print -quit"
    ).strip()
    assert intent, intent
    machine.succeed("test \"$(stat -c '%u:%a' " + shlex.quote(intent) + ")\" = 0:600")
    machine.fail("su -s /bin/sh alice -c " + shlex.quote("cat " + intent))
    machine.succeed("test -z \"$(find /var/lib/theblob/materialization-admissions -maxdepth 1 -type f -print -quit)\"")

    # Completion is verification only: it must not realize a missing result.
    status, early_output = root_complete()
    assert status != 0, (status, early_output)
    assert "materialization completion rejected" in early_output.lower(), early_output
    machine.succeed("test ! -e " + shlex.quote(expected))
    machine.succeed("test -z \"$(find /var/lib/theblob/materialization-admissions -maxdepth 1 -type f -print -quit)\"")

    # The actual realization is deliberately performed as a normal, non-root user
    # against the exact derivation target root committed to before the build.
    alice_build = (
        "HOME=/home/alice " + NIX
        + " build --no-link --no-write-lock-file " + shlex.quote(target)
    )
    machine.succeed("su -s /bin/sh alice -c " + shlex.quote(alice_build))
    machine.succeed("test -e " + shlex.quote(expected))
    machine.succeed("grep -qx BLOB_MATERIALIZATION_ADMISSION_OK " + shlex.quote(expected + "/blob-marker"))

    # Completion accepts only the operation id. Root reloads its immutable intent,
    # verifies the precomputed output and deriver, then publishes the admission.
    status, complete_output = root_complete()
    assert status == 0, (status, complete_output)
    assert field(complete_output, "closure") == expected, complete_output
    assert "provenance=derivation:" + derivation in complete_output, complete_output
    assert "provenance=expected-output:" + expected in complete_output, complete_output
    assert "provenance=verified-deriver:" + derivation in complete_output, complete_output
    assert "provenance=verified-nar-hash:" in complete_output, complete_output

    admission = machine.succeed(
        "find /var/lib/theblob/materialization-admissions -maxdepth 1 -type f -name '*.admission' -print -quit"
    ).strip()
    assert admission, admission
    machine.succeed("test \"$(stat -c '%u:%a' " + shlex.quote(admission) + ")\" = 0:600")
    machine.fail("su -s /bin/sh alice -c " + shlex.quote("cat " + admission))
    machine.succeed("grep -q 'theblob-materialization-admission-v1' " + shlex.quote(admission))

    completed_intent = machine.succeed(
        "find /var/lib/theblob/materialization-intents/completed -maxdepth 1 -type f -name '*.intent' -print -quit"
    ).strip()
    assert completed_intent, completed_intent
    machine.succeed("test ! -e " + shlex.quote(intent))

    # A decoy output can be realized by the materializer, but cannot be substituted
    # at completion because the authority never accepts an output path from it.
    decoy_status, decoy_begin = root_begin(
        operation="op:blob-materialization-admission-decoy",
        attribute="packages.x86_64-linux.decoy",
    )
    assert decoy_status == 0, (decoy_status, decoy_begin)
    decoy_target = field(decoy_begin, "build-target")
    decoy_output = field(decoy_begin, "expected-output")
    machine.succeed(
        "su -s /bin/sh alice -c "
        + shlex.quote("HOME=/home/alice " + NIX + " build --no-link --no-write-lock-file " + shlex.quote(decoy_target))
    )
    machine.succeed("grep -qx DECOY " + shlex.quote(decoy_output + "/blob-marker"))

    # Replaying a completed operation cannot create or mutate another admission.
    status, replay_output = root_complete()
    assert status != 0, (status, replay_output)
    assert "IntentMissing" in replay_output or "intent" in replay_output.lower(), replay_output
    assert machine.succeed(
        "find /var/lib/theblob/materialization-admissions -maxdepth 1 -type f -name '*.admission' | wc -l"
    ).strip() == "1"
  '';
}
