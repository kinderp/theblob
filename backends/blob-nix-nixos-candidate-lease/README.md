# `blob-nix-nixos-candidate-lease`

Root-owned quiescence protocol shared by trusted-candidate enqueue and candidate retirement.

The module closes the race where lifecycle cleanup observes no durable begin job while an enqueue has already loaded the candidate manifest but has not yet published its job.

## Protocol

Enqueue must acquire a lease **before** reading candidate selection state:

```text
check no retiring/retired barrier
  -> publish 0600 active lease
  -> fsync active directory
  -> recheck barrier
  -> only then read manifest/source identity
  -> publish durable begin job
  -> release lease
```

Retirement proceeds monotonically:

```text
publish 0600 retiring barrier
  -> fsync
  -> require no matching active lease
  -> retire candidate selection state
  -> require quiescence again
  -> retiring -> retired
```

`retired` is a permanent tombstone. Once it exists, the same manifest id can never become enqueueable again.

## Race result

- If enqueue publishes its lease first, retirement observes it and returns `Busy`; source retention is unchanged.
- If retirement publishes its barrier first, an enqueue either sees it before lease creation or catches it in the mandatory post-create recheck. In both cases it fails before candidate state is read.
- A crash can leave an active lease behind. That is intentionally a retention leak, not deletion pressure. It may be reclaimed only at the existing daemon-startup recovery point after systemd has destroyed the prior service control group.

All directories are root-owned mode 0700 and records are mode 0600. Ambiguous layout, ownership or record state fails closed.
