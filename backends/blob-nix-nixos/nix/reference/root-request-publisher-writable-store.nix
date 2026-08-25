{ ... }:
{
  nodes.machine = { ... }: {
    # runNixOSTest normally mounts the host store read-only. The publisher's
    # production readiness probe deliberately measures writable /nix/store
    # capacity, so this checkpoint must use the QEMU writable-store overlay.
    virtualisation.writableStore = true;
  };
}
