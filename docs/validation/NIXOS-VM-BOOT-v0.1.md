# NixOS VM boot smoke v0.1 validation checkpoint

This checkpoint proves that the first Linux Pilot candidate generated through `SystemSpec -> NixOS` is not only evaluable and buildable but reaches NixOS userspace under QEMU/KVM.

A validation-only module adds a one-shot systemd service that:

1. starts as part of `multi-user.target`;
2. writes `BLOB_VM_BOOT_OK` to the VM serial console;
3. requests a clean poweroff.

CI builds the isolated `blob-pilot-smoke` VM configuration and runs its generated NixOS VM runner with a serial console:

```text
QEMU_OPTS="-nographic -serial mon:stdio"
QEMU_KERNEL_PARAMS="console=ttyS0"
```

The checkpoint passes only when:

- the marker appears on the captured serial console;
- the VM exits cleanly before the timeout;
- all normal core, Slint, WASIp2 and NixOS-evaluation jobs remain green.

The smoke service is deliberately not part of the user's semantic `SystemSpec` or normal `blob-pilot` configuration. It is validation instrumentation only.

A successful result establishes this chain:

```text
SystemSpec
   -> deterministic Rust translation
   -> generated NixOS module
   -> pinned NixOS module evaluation
   -> immutable VM build
   -> Linux kernel boot
   -> systemd userspace
   -> multi-user target
   -> BLOB_VM_BOOT_OK
```

The next Linux Pilot step is to expose controlled NixOS candidate operations (`build`, `dry-activate`, `test`, `build-vm`) through a Blob backend API and attach their predicted/observed effects to Temporal/Causal history before touching a physical test node.
