{ pkgs, lib, ... }:
let
  busName = "org.theblob.NixOsMaterialization";
  objectPath = "/org/theblob/NixOsMaterialization";
  interfaceName = "org.theblob.NixOsMaterialization1";

  trustedBaseModule = pkgs.writeText "blob-async-begin-base.nix" (builtins.readFile ./base.nix);

  rootHarnesses = pkgs.stdenv.mkDerivation {
    pname = "blob-async-materialization-begin-vm";
    version = "0.1.0";
    src = lib.cleanSource ../../../..;
    nativeBuildInputs = [ pkgs.cargo pkgs.rustc ];
    buildPhase = ''
      runHook preBuild
      export HOME="$TMPDIR/home"
      export CARGO_HOME="$TMPDIR/cargo-home"
      mkdir -p "$HOME" "$CARGO_HOME"
      cargo build --offline --release \
        -p blob-nix-nixos-materialization-begin-queue \
        --example root_async_materialization_begin_vm
      cargo build --offline --release \
        -p blob-nix-nixos-candidate-producer \
        --example root_systemspec_candidate_producer_vm
      cargo build --offline --release \
        -p blob-nix-nixos-candidate-producer \
        --example render_reference_system_spec
      runHook postBuild
    '';
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/bin"
      cp target/release/examples/root_async_materialization_begin_vm \
        "$out/bin/blob-async-materialization-begin-vm"
      cp target/release/examples/root_systemspec_candidate_producer_vm \
        "$out/bin/blob-systemspec-candidate-producer-vm"
      cp target/release/examples/render_reference_system_spec \
        "$out/bin/blob-render-reference-system-spec"
      runHook postInstall
    '';
  };

  blobDbusPolicy = pkgs.writeTextDir "share/dbus-1/system.d/org.theblob.NixOsMaterialization.conf" ''
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
    name = "blob-async-materialization-begin-service";
    executable = true;
    text = ''
      #!${python}/bin/python3
      import subprocess
      import sys
      import threading
      import time
      from gi.repository import Gio, GLib

      BUS_NAME = ${builtins.toJSON busName}
      OBJECT_PATH = ${builtins.toJSON objectPath}
      INTERFACE = ${builtins.toJSON interfaceName}
      HARNESS = ${builtins.toJSON "${rootHarnesses}/bin/blob-async-materialization-begin-vm"}
      NIX = ${builtins.toJSON "${pkgs.nix}/bin/nix"}
      NIX_STORE = ${builtins.toJSON "${pkgs.nix}/bin/nix-store"}

      INTROSPECTION = f"""
      <node>
        <interface name='{INTERFACE}'>
          <method name='EnqueueBegin'>
            <arg type='s' name='manifest_id' direction='in'/>
            <arg type='s' name='request_id' direction='out'/>
            <arg type='s' name='operation' direction='out'/>
          </method>
          <method name='GetBeginStatus'>
            <arg type='s' name='request_id' direction='in'/>
            <arg type='s' name='state' direction='out'/>
            <arg type='s' name='operation' direction='out'/>
            <arg type='s' name='evidence' direction='out'/>
          </method>
        </interface>
      </node>
      """

      def run_cli(arguments, check=True):
          result = subprocess.run(
              [HARNESS] + arguments,
              stdin=subprocess.DEVNULL,
              stdout=subprocess.PIPE,
              stderr=subprocess.PIPE,
              text=True,
              check=False,
          )
          if check and result.returncode != 0:
              detail = result.stderr.strip().replace("\n", " | ")
              raise RuntimeError(detail[-1800:] or "async begin harness rejected")
          return result

      def field(evidence, key):
          for line in evidence.splitlines():
              if line.startswith(key + "="):
                  return line.split("=", 1)[1]
          raise RuntimeError("missing " + key)

      def unix_uid(connection, sender):
          reply = connection.call_sync(
              "org.freedesktop.DBus",
              "/org/freedesktop/DBus",
              "org.freedesktop.DBus",
              "GetConnectionUnixUser",
              GLib.Variant("(s)", (sender,)),
              GLib.VariantType.new("(u)"),
              Gio.DBusCallFlags.NONE,
              -1,
              None,
          )
          return reply.unpack()[0]

      def worker_loop():
          while True:
              result = run_cli([
                  "--mode", "work-one",
                  "--nix", NIX,
                  "--nix-store", NIX_STORE,
              ], check=False)
              if result.returncode != 0:
                  print(
                      "BLOB_ASYNC_BEGIN_WORKER_ERROR " + result.stderr.strip().replace("\n", " | "),
                      file=sys.stderr,
                      flush=True,
                  )
              if "none=true" in result.stdout:
                  time.sleep(0.2)
              else:
                  time.sleep(0.05)

      def on_method_call(connection, sender, object_path, interface_name, method_name, parameters, invocation):
          if not sender or not sender.startswith(":"):
              invocation.return_dbus_error(
                  "org.theblob.Error.InvalidSender",
                  "The system bus did not provide a unique sender name",
              )
              return
          try:
              uid = unix_uid(connection, sender)
              if method_name == "EnqueueBegin":
                  manifest_id = parameters.unpack()[0]
                  evidence = run_cli([
                      "--mode", "enqueue",
                      "--sender", sender,
                      "--uid", str(uid),
                      "--manifest-id", manifest_id,
                  ]).stdout.strip()
                  request_id = field(evidence, "request-id")
                  operation = field(evidence, "operation")
                  invocation.return_value(GLib.Variant("(ss)", (request_id, operation)))
                  return

              if method_name == "GetBeginStatus":
                  request_id = parameters.unpack()[0]
                  evidence = run_cli([
                      "--mode", "status",
                      "--request-id", request_id,
                      "--uid", str(uid),
                  ]).stdout.strip()
                  state = field(evidence, "state")
                  operation = field(evidence, "operation")
                  invocation.return_value(GLib.Variant("(sss)", (state, operation, evidence)))
                  return

              invocation.return_dbus_error(
                  "org.theblob.Error.UnsupportedMethod",
                  "Unsupported async materialization method",
              )
          except Exception as error:
              invocation.return_dbus_error(
                  "org.theblob.Error.MaterializationRejected",
                  str(error),
              )

      recovered = run_cli(["--mode", "recover"]).stdout.strip()
      print("BLOB_ASYNC_BEGIN_STARTUP " + recovered, flush=True)

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

      worker = threading.Thread(target=worker_loop, name="blob-async-begin-worker", daemon=True)
      worker.start()
      GLib.MainLoop().run()
    '';
  };
in
{
  name = "blob-async-materialization-begin";

  nodes.machine = { ... }: {
    nix.settings.experimental-features = [ "nix-command" "flakes" ];
    nix.settings.substituters = lib.mkForce [ ];
    system.extraDependencies = [ pkgs.path trustedBaseModule ];

    services.dbus.packages = [ blobDbusPolicy ];
    users.users.alice = { isNormalUser = true; uid = 1000; };
    users.users.bob = { isNormalUser = true; uid = 1001; };
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
      "d /var/lib/theblob/materialization-begin-jobs 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs/queued 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs/running 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs/completed 0700 root root -"
      "d /var/lib/theblob/materialization-begin-jobs/failed 0700 root root -"
      "d /nix/var/nix/gcroots/theblob-candidate-sources 0700 root root -"
      "d /nix/var/nix/gcroots/theblob-materializations 0700 root root -"
    ];

    systemd.services.blob-async-materialization-begin = {
      description = "The Blob durable asynchronous materialization begin daemon";
      wantedBy = [ "multi-user.target" ];
      after = [ "dbus.service" "systemd-tmpfiles-setup.service" ];
      requires = [ "dbus.service" ];
      serviceConfig = {
        Type = "simple";
        User = "root";
        Group = "root";
        ExecStart = "${rootService}";
        Restart = "always";
        RestartSec = "100ms";
        KillMode = "control-group";
        NoNewPrivileges = true;
        PrivateTmp = true;
      };
    };

    virtualisation.writableStore = true;
    virtualisation.writableStoreUseTmpfs = false;
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
    HARNESS = "${rootHarnesses}/bin/blob-async-materialization-begin-vm"
    PRODUCER = "${rootHarnesses}/bin/blob-systemspec-candidate-producer-vm"
    RENDER_SPEC = "${rootHarnesses}/bin/blob-render-reference-system-spec"
    NIX = "${pkgs.nix}/bin/nix"
    NIX_STORE = "${pkgs.nix}/bin/nix-store"
    NIXPKGS = "${pkgs.path}"
    BASE = "${trustedBaseModule}"
    STAGING = "/var/lib/theblob/candidate-source-staging"
    JOBS = "/var/lib/theblob/materialization-begin-jobs"
    INTENTS = "/var/lib/theblob/materialization-intents"
    DERIVATION_ROOTS = "/nix/var/nix/gcroots/theblob-materializations"

    def field(output, key):
        match = re.search(r"^" + re.escape(key) + r"=(.+)$", output, re.MULTILINE)
        assert match is not None, (key, output)
        return match.group(1).strip()

    def root_produce_manifest():
        spec = machine.succeed(RENDER_SPEC)
        command = (
            PRODUCER
            + " --sender :0.33"
            + " --nix " + shlex.quote(NIX)
            + " --nixpkgs-source " + shlex.quote(NIXPKGS)
            + " --base-module " + shlex.quote(BASE)
            + " --staging-root " + shlex.quote(STAGING)
        )
        status, output = machine.execute(command + " <<'BLOB_SPEC'\n" + spec + "BLOB_SPEC\n")
        assert status == 0, (status, output)
        return field(output, "manifest-id")

    def bus_call(user, method, signature, *args):
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

    def enqueue(user, manifest_id):
        status, output = bus_call(user, "EnqueueBegin", "s", manifest_id)
        if status != 0:
            return status, output, None, None
        values = quoted(output)
        request_id = next((value for value in values if value.startswith("begin-request:")), None)
        operation = next((value for value in values if value.startswith("op:materialize-")), None)
        return status, output, request_id, operation

    def status(user, request_id):
        code, output = bus_call(user, "GetBeginStatus", "s", request_id)
        if code != 0:
            return code, output, None, None
        values = quoted(output)
        state = next((value for value in values if value in ("queued", "running", "completed", "failed")), None)
        operation = next((value for value in values if value.startswith("op:materialize-")), None)
        return code, output, state, operation

    def count_state(state):
        return int(machine.succeed(
            "find " + JOBS + "/" + state + " -maxdepth 1 -type f -name '*.job' | wc -l"
        ).strip())

    def op_gcroot(operation):
        return DERIVATION_ROOTS + "/operation-" + operation.encode("utf-8").hex() + "-derivation"

    machine.start()
    machine.wait_for_unit("multi-user.target")
    machine.wait_for_unit("blob-async-materialization-begin.service")

    manifest_id = root_produce_manifest()
    assert manifest_id.startswith("manifest:systemspec-"), manifest_id

    # Enqueue returns under the normal D-Bus client timeout. No Nix derivation
    # resolution is performed in this RPC.
    code, output, request_id, operation = enqueue("alice", manifest_id)
    assert code == 0 and request_id is not None and operation is not None, (code, output)
    assert request_id.startswith("begin-request:"), request_id
    assert operation.startswith("op:materialize-"), operation

    # A different local user cannot inspect Alice's durable request even if the
    # high-entropy request id is known.
    code, output, _, _ = status("bob", request_id)
    assert code != 0, (code, output)

    # Job records are root-owned and not directly readable by the requester.
    machine.wait_until_succeeds("test $(find " + JOBS + " -mindepth 2 -type f -name '*.job' | wc -l) -eq 1")
    job_path = machine.succeed("find " + JOBS + " -mindepth 2 -type f -name '*.job' -print -quit").strip()
    machine.succeed("test \"$(stat -c %u:%a " + shlex.quote(job_path) + ")\" = 0:600")
    machine.fail("su -s /bin/sh alice -c " + shlex.quote("cat " + job_path))

    # Wait until the worker has performed the first exact resolution and retained
    # the preallocated operation's derivation. The second authority resolution is
    # intentionally long, giving a deterministic crash window.
    gcroot = op_gcroot(operation)
    machine.wait_until_succeeds("test -L " + shlex.quote(gcroot), timeout=600)
    code, output, observed_state, observed_operation = status("alice", request_id)
    assert code == 0 and observed_state == "running", (code, output, observed_state)
    assert observed_operation == operation, (observed_operation, operation)

    old_pid = machine.succeed(
        "systemctl show -p MainPID --value blob-async-materialization-begin.service"
    ).strip()
    assert old_pid != "0", old_pid

    # Kill the entire service control group. This kills the Python daemon and its
    # currently running worker/Nix client. The running job is left durable.
    machine.succeed(
        "systemctl kill --kill-who=all --signal=KILL blob-async-materialization-begin.service"
    )
    machine.wait_until_succeeds("systemctl is-active --quiet blob-async-materialization-begin.service", timeout=120)
    machine.wait_until_succeeds(
        "test \"$(systemctl show -p MainPID --value blob-async-materialization-begin.service)\" != \"0\""
    )
    new_pid = machine.succeed(
        "systemctl show -p MainPID --value blob-async-materialization-begin.service"
    ).strip()
    assert new_pid != old_pid, (old_pid, new_pid)

    # Startup recovery requeues the stranded running record; the new worker claims
    # the same request and same preallocated operation. No second operation exists.
    machine.wait_until_succeeds(
        "busctl --system status " + DEST + " >/dev/null 2>&1",
        timeout=120,
    )
    code, output, state_after_restart, operation_after_restart = status("alice", request_id)
    assert code == 0, (code, output)
    assert state_after_restart in ("queued", "running", "completed"), state_after_restart
    assert operation_after_restart == operation, (operation_after_restart, operation)

    # The recovered worker eventually creates exactly one pending materialization
    # intent and moves the queue request to completed.
    def completed():
        code, output, state, observed_operation = status("alice", request_id)
        return code == 0 and state == "completed" and observed_operation == operation

    machine.wait_until_succeeds(
        "su -s /bin/sh alice -c "
        + shlex.quote(
            "busctl --system call " + DEST + " " + PATH + " " + IFACE
            + " GetBeginStatus s " + shlex.quote(request_id)
        )
        + " | grep -q 'completed'",
        timeout=900,
    )
    assert completed(), status("alice", request_id)

    assert count_state("queued") == 0
    assert count_state("running") == 0
    assert count_state("completed") == 1
    assert count_state("failed") == 0

    pending_count = int(machine.succeed(
        "find " + INTENTS + "/pending -maxdepth 1 -type f -name '*.intent' | wc -l"
    ).strip())
    assert pending_count == 1, pending_count
    machine.succeed("test -L " + shlex.quote(gcroot))
    assert machine.succeed("readlink " + shlex.quote(gcroot)).strip().endswith(".drv")

    # Restarting after terminal completion does not requeue or execute the job.
    completed_job = machine.succeed(
        "find " + JOBS + "/completed -maxdepth 1 -type f -name '*.job' -print -quit"
    ).strip()
    completed_mtime = machine.succeed("stat -c %Y " + shlex.quote(completed_job)).strip()
    machine.succeed("systemctl restart blob-async-materialization-begin.service")
    machine.wait_for_unit("blob-async-materialization-begin.service")
    code, output, final_state, final_operation = status("alice", request_id)
    assert code == 0 and final_state == "completed", (code, output, final_state)
    assert final_operation == operation
    assert machine.succeed("stat -c %Y " + shlex.quote(completed_job)).strip() == completed_mtime
    assert count_state("completed") == 1
    assert int(machine.succeed(
        "find " + INTENTS + "/pending -maxdepth 1 -type f -name '*.intent' | wc -l"
    ).strip()) == 1

    code, output, _, _ = status("alice", "begin-request:does-not-exist")
    assert code != 0, (code, output)
  '';
}
