{ pkgs, lib, ... }:
let
  busName = "org.theblob.NixOsCandidate";
  objectPath = "/org/theblob/NixOsCandidate";
  interfaceName = "org.theblob.NixOsCandidate1";

  trustedBaseModule = pkgs.writeText "blob-candidate-base.nix" (builtins.readFile ./base.nix);
  expectedGenerated = pkgs.writeText "blob-expected-generated.nix" (builtins.readFile ./generated.nix);

  rootHarnesses = pkgs.stdenv.mkDerivation {
    pname = "blob-systemspec-candidate-producer-vm";
    version = "0.1.0";
    src = lib.cleanSource ../../../..;
    nativeBuildInputs = [ pkgs.cargo pkgs.rustc ];
    buildPhase = ''
      runHook preBuild
      export HOME="$TMPDIR/home"
      export CARGO_HOME="$TMPDIR/cargo-home"
      mkdir -p "$HOME" "$CARGO_HOME"
      cargo build --offline --release \
        -p blob-nix-nixos-candidate-producer \
        --example root_systemspec_candidate_producer_vm
      cargo build --offline --release \
        -p blob-nix-nixos-candidate-producer \
        --example render_reference_system_spec
      cargo build --offline --release \
        -p blob-nix-nixos-materialization-begin \
        --example root_materialization_begin_vm
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/bin"
      cp target/release/examples/root_systemspec_candidate_producer_vm \
        "$out/bin/blob-systemspec-candidate-producer-vm"
      cp target/release/examples/render_reference_system_spec \
        "$out/bin/blob-render-reference-system-spec"
      cp target/release/examples/root_materialization_begin_vm \
        "$out/bin/blob-materialization-begin-vm"
      runHook postInstall
    '';
  };

  blobDbusPolicy = pkgs.writeTextDir "share/dbus-1/system.d/org.theblob.NixOsCandidate.conf" ''
    <!DOCTYPE busconfig PUBLIC
      "-//freedesktop//DTD D-BUS Bus Configuration 1.0//EN"
      "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
    <busconfig>
      <policy user="root">
        <allow own="${busName}"/>
      </policy>
      <policy context="default">
        <allow send_destination="${busName}"/>
      </policy>
    </busconfig>
  '';

  python = pkgs.python3.withPackages (ps: [ ps.pygobject3 ]);
  rootService = pkgs.writeTextFile {
    name = "blob-systemspec-candidate-producer-service";
    executable = true;
    text = ''
      #!${python}/bin/python3
      import subprocess
      from gi.repository import Gio, GLib

      BUS_NAME = ${builtins.toJSON busName}
      OBJECT_PATH = ${builtins.toJSON objectPath}
      INTERFACE = ${builtins.toJSON interfaceName}
      PRODUCER = ${builtins.toJSON "${rootHarnesses}/bin/blob-systemspec-candidate-producer-vm"}
      BEGIN = ${builtins.toJSON "${rootHarnesses}/bin/blob-materialization-begin-vm"}
      NIX = ${builtins.toJSON "${pkgs.nix}/bin/nix"}
      NIX_STORE = ${builtins.toJSON "${pkgs.nix}/bin/nix-store"}
      NIXPKGS = ${builtins.toJSON "${pkgs.path}"}
      BASE_MODULE = ${builtins.toJSON "${trustedBaseModule}"}
      STAGING = "/var/lib/theblob/candidate-source-staging"

      INTROSPECTION = f"""
      <node>
        <interface name='{INTERFACE}'>
          <method name='PrepareCandidate'>
            <arg type='s' name='canonical_system_spec' direction='in'/>
            <arg type='s' name='manifest_id' direction='out'/>
            <arg type='s' name='evidence' direction='out'/>
          </method>
          <method name='Begin'>
            <arg type='s' name='manifest_id' direction='in'/>
            <arg type='s' name='operation' direction='out'/>
            <arg type='s' name='evidence' direction='out'/>
          </method>
        </interface>
      </node>
      """

      def run_producer(sender, canonical_spec):
          result = subprocess.run(
              [
                  PRODUCER,
                  "--sender", sender,
                  "--nix", NIX,
                  "--nixpkgs-source", NIXPKGS,
                  "--base-module", BASE_MODULE,
                  "--staging-root", STAGING,
              ],
              input=canonical_spec,
              stdout=subprocess.PIPE,
              stderr=subprocess.PIPE,
              text=True,
              check=False,
          )
          if result.returncode != 0:
              detail = result.stderr.strip().replace("\n", " | ")
              raise RuntimeError(detail[-1800:] or "candidate producer rejected")
          return result.stdout.strip()

      def run_begin(manifest_id):
          result = subprocess.run(
              [
                  BEGIN,
                  "--mode", "begin",
                  "--manifest-id", manifest_id,
                  "--nix", NIX,
                  "--nix-store", NIX_STORE,
              ],
              stdin=subprocess.DEVNULL,
              stdout=subprocess.PIPE,
              stderr=subprocess.PIPE,
              text=True,
              check=False,
          )
          if result.returncode != 0:
              detail = result.stderr.strip().replace("\n", " | ")
              raise RuntimeError(detail[-1800:] or "materialization begin rejected")
          return result.stdout.strip()

      def field(evidence, key, prefix=None):
          for line in evidence.splitlines():
              if line.startswith(key + "="):
                  value = line.split("=", 1)[1]
                  if prefix is None or value.startswith(prefix):
                      return value
          raise RuntimeError("missing valid " + key)

      def on_method_call(connection, sender, object_path, interface_name, method_name, parameters, invocation):
          if not sender or not sender.startswith(":"):
              invocation.return_dbus_error(
                  "org.theblob.Error.InvalidSender",
                  "The system bus did not provide a unique sender name",
              )
              return
          try:
              if method_name == "PrepareCandidate":
                  canonical_spec = parameters.unpack()[0]
                  evidence = run_producer(sender, canonical_spec)
                  manifest_id = field(evidence, "manifest-id", "manifest:systemspec-")
                  evidence = "sender=" + sender + "\n" + evidence
                  invocation.return_value(GLib.Variant("(ss)", (manifest_id, evidence)))
                  return

              if method_name == "Begin":
                  manifest_id = parameters.unpack()[0]
                  evidence = run_begin(manifest_id)
                  operation = field(evidence, "operation", "op:materialize-")
                  evidence = "sender=" + sender + "\n" + evidence
                  invocation.return_value(GLib.Variant("(ss)", (operation, evidence)))
                  return

              invocation.return_dbus_error(
                  "org.theblob.Error.UnsupportedMethod",
                  "Unsupported candidate method",
              )
          except Exception as error:
              invocation.return_dbus_error(
                  "org.theblob.Error.CandidateRejected",
                  str(error),
              )

      node = Gio.DBusNodeInfo.new_for_xml(INTROSPECTION)
      connection = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
      reply = connection.call_sync(
          "org.freedesktop.DBus",
          "/org/freedesktop/DBus",
          "org.freedesktop.DBus",
          "RequestName",
          GLib.Variant("(su)", (BUS_NAME, 4)),
          GLib.VariantType.new("(u)"),
          Gio.DBusCallFlags.NONE,
          -1,
          None,
      )
      if reply.unpack()[0] != 1:
          raise RuntimeError("failed to own " + BUS_NAME)
      registration = connection.register_object(
          OBJECT_PATH,
          node.interfaces[0],
          on_method_call,
          None,
          None,
      )
      if registration == 0:
          raise RuntimeError("failed to register D-Bus object")
      GLib.MainLoop().run()
    '';
  };
in
{
  name = "blob-systemspec-candidate-producer";

  nodes.machine = { ... }: {
    nix.settings.experimental-features = [ "nix-command" "flakes" ];
    nix.settings.substituters = lib.mkForce [ ];
    system.extraDependencies = [ pkgs.path trustedBaseModule expectedGenerated ];

    services.dbus.packages = [ blobDbusPolicy ];
    users.users.alice = { isNormalUser = true; uid = 1000; };

    environment.systemPackages = [ pkgs.dbus pkgs.nix rootHarnesses ];
    systemd.tmpfiles.rules = [
      "d /var/lib/theblob 0700 root root -"
      "d /var/lib/theblob/materialization-candidates 0700 root root -"
      "d /var/lib/theblob/candidate-manifest-receipts 0700 root root -"
      "d /var/lib/theblob/candidate-source-staging 0700 root root -"
      "d /var/lib/theblob/materialization-intents 0700 root root -"
      "d /var/lib/theblob/materialization-intents/pending 0700 root root -"
      "d /var/lib/theblob/materialization-intents/completed 0700 root root -"
      "d /var/lib/theblob/materialization-admissions 0700 root root -"
      "d /nix/var/nix/gcroots/theblob-candidate-sources 0700 root root -"
      "d /nix/var/nix/gcroots/theblob-materializations 0700 root root -"
    ];

    systemd.services.blob-systemspec-candidate-producer = {
      description = "The Blob canonical SystemSpec candidate producer";
      wantedBy = [ "multi-user.target" ];
      after = [ "dbus.service" "systemd-tmpfiles-setup.service" ];
      requires = [ "dbus.service" ];
      serviceConfig = {
        Type = "simple";
        User = "root";
        Group = "root";
        ExecStart = "${rootService}";
        Restart = "on-failure";
        RestartSec = "100ms";
        NoNewPrivileges = true;
        PrivateTmp = true;
      };
    };

    virtualisation.diskSize = 12288;
    virtualisation.memorySize = 2048;
    virtualisation.cores = 2;
  };

  testScript = ''
    import re
    import shlex

    DEST = "${busName}"
    PATH = "${objectPath}"
    IFACE = "${interfaceName}"
    RENDER_SPEC = "${rootHarnesses}/bin/blob-render-reference-system-spec"
    EXPECTED_GENERATED = "${expectedGenerated}"
    NIXPKGS = "${pkgs.path}"
    MANIFESTS = "/var/lib/theblob/materialization-candidates"
    RECEIPTS = "/var/lib/theblob/candidate-manifest-receipts"
    SOURCE_ROOTS = "/nix/var/nix/gcroots/theblob-candidate-sources"
    INTENTS = "/var/lib/theblob/materialization-intents"
    DERIVATION_ROOTS = "/nix/var/nix/gcroots/theblob-materializations"

    def call(user, method, signature, *args):
        tail = " " + signature
        if args:
            tail += " " + " ".join(shlex.quote(value) for value in args)
        inner = (
            "busctl --system call " + DEST + " " + PATH + " " + IFACE
            + " " + method + tail + " 2>&1"
        )
        return machine.execute("su -s /bin/sh " + user + " -c " + shlex.quote(inner))

    def quoted(output):
        return re.findall(r'"([^"\\]*(?:\\.[^"\\]*)*)"', output)

    def unescape(value):
        return value.replace("\\n", "\n").replace("\\\"", "\"")

    def evidence_from(output, marker):
        values = quoted(output)
        evidence = next((unescape(value) for value in values if marker in value), None)
        assert evidence is not None, (marker, output)
        return evidence

    def field(evidence, key):
        match = re.search(r"^" + re.escape(key) + r"=(.+)$", evidence, re.MULTILINE)
        assert match is not None, (key, evidence)
        return match.group(1).strip()

    machine.start()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("blob-systemspec-candidate-producer.service")

    spec = machine.succeed(RENDER_SPEC)
    assert spec.startswith("theblob-system-spec-v1\n"), spec
    assert spec.endswith("\n"), spec

    # The public producer accepts exactly one semantic SystemSpec string. Extra
    # caller-controlled source/native arguments are rejected by D-Bus itself.
    status, output = call("alice", "PrepareCandidate", "ss", spec, "/nix/store/evil")
    assert status != 0, (status, output)
    machine.succeed("test -z \"$(find " + MANIFESTS + " -maxdepth 1 -type f -print -quit)\"")

    # Native configuration cannot be smuggled into the canonical semantic model.
    noncanonical = spec + "raw-nix=696d70757265\n"
    status, output = call("alice", "PrepareCandidate", "s", noncanonical)
    assert status != 0, (status, output)
    machine.succeed("test -z \"$(find " + MANIFESTS + " -maxdepth 1 -type f -print -quit)\"")
    machine.succeed("test -z \"$(find " + SOURCE_ROOTS + " -maxdepth 1 -type l -print -quit)\"")

    # A valid SystemSpec is revalidated and translated in root. The caller does
    # not choose manifest id, candidate id, Nix source or installable attribute.
    status, output = call("alice", "PrepareCandidate", "s", spec)
    assert status == 0, (status, output)
    evidence = evidence_from(output, "candidate=customer" if False else "candidate=")
    manifest_id = field(evidence, "manifest-id")
    candidate = field(evidence, "candidate")
    system_spec = field(evidence, "system-spec")
    source = field(evidence, "source")
    installable = field(evidence, "installable")
    manifest_path = field(evidence, "manifest")
    receipt_path = field(evidence, "receipt")
    source_gcroot = field(evidence, "source-gcroot")
    causal_id = field(evidence, "causal-id")

    assert manifest_id.startswith("manifest:systemspec-"), manifest_id
    assert candidate.startswith("candidate:systemspec-"), candidate
    assert system_spec == "system:manifest-producer-vm", system_spec
    assert source.startswith("/nix/store/"), source
    assert installable == "nixosConfigurations.blob-pilot.config.system.build.toplevel", installable
    assert causal_id.startswith("causal:candidate-manifest-"), causal_id
    assert field(evidence, "sender").startswith(":"), evidence

    machine.succeed("test \"$(stat -c %u:%a " + shlex.quote(manifest_path) + ")\" = 0:600")
    machine.succeed("test \"$(stat -c %u:%a " + shlex.quote(receipt_path) + ")\" = 0:600")
    machine.fail("su -s /bin/sh alice -c " + shlex.quote("cat " + manifest_path))
    machine.fail("su -s /bin/sh alice -c " + shlex.quote("cat " + receipt_path))
    machine.succeed("grep -qx 'theblob-trusted-materialization-candidate-v1' " + shlex.quote(manifest_path))
    machine.succeed("grep -qx 'theblob-candidate-manifest-receipt-v1' " + shlex.quote(receipt_path))

    machine.succeed("test -L " + shlex.quote(source_gcroot))
    assert machine.succeed("readlink " + shlex.quote(source_gcroot)).strip() == source
    machine.succeed("cmp " + shlex.quote(source + "/generated.nix") + " " + shlex.quote(EXPECTED_GENERATED))
    machine.succeed("grep -Fq " + shlex.quote("path:" + NIXPKGS) + " " + shlex.quote(source + "/flake.nix"))
    machine.succeed("grep -Fq 'modules = [ ./base.nix ./generated.nix ];' " + shlex.quote(source + "/flake.nix"))
    machine.fail("grep -Fq 'builtins.getEnv' " + shlex.quote(source + "/flake.nix"))
    machine.fail("grep -Fq -- '--impure' " + shlex.quote(source + "/flake.nix"))

    # Compose directly into ADR-0037. The Begin caller supplies only the manifest
    # id; every Nix/native field must round-trip from the producer-owned manifest.
    status, begin_output = call("alice", "Begin", "s", manifest_id)
    assert status == 0, (status, begin_output)
    begin_evidence = evidence_from(begin_output, "derivation=")
    operation = field(begin_evidence, "operation")
    assert operation.startswith("op:materialize-"), operation
    assert field(begin_evidence, "candidate") == candidate, begin_evidence
    assert field(begin_evidence, "system-spec") == system_spec, begin_evidence
    assert field(begin_evidence, "source") == source, begin_evidence
    assert field(begin_evidence, "attribute") == installable, begin_evidence
    derivation = field(begin_evidence, "derivation")
    expected = field(begin_evidence, "expected-output")
    assert field(begin_evidence, "build-target") == derivation + "^out", begin_evidence

    pending = machine.succeed(
        "find " + INTENTS + "/pending -maxdepth 1 -type f -name '*.intent' -print -quit"
    ).strip()
    assert pending, pending
    machine.succeed("test \"$(stat -c %u:%a " + shlex.quote(pending) + ")\" = 0:600")
    machine.fail("su -s /bin/sh alice -c " + shlex.quote("cat " + pending))
    machine.succeed("test \"$(find " + DERIVATION_ROOTS + " -maxdepth 1 -type l | wc -l)\" = 1")
    assert machine.succeed("readlink $(find " + DERIVATION_ROOTS + " -maxdepth 1 -type l -print -quit)").strip() == derivation

    # The candidate source remains retained by its manifest until a later
    # lifecycle/GC checkpoint explicitly retires unused manifests.
    machine.succeed("test -L " + shlex.quote(source_gcroot))
    machine.succeed("test ! -e " + shlex.quote(expected))
  '';
}
