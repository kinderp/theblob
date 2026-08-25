{ ... }:
{
  nodes.machine = { ... }: {
    # runNixOSTest normally mounts the host store read-only. The publisher's
    # production readiness probe deliberately measures writable /nix/store
    # capacity, so this checkpoint uses the QEMU writable-store overlay.
    # Keep the overlay on the VM filesystem instead of the default tmpfs so
    # `df /nix/store` observes the sparse 12 GiB disk rather than VM RAM.
    virtualisation.writableStore = true;
    virtualisation.writableStoreUseTmpfs = false;
  };
}
