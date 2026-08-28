#![forbid(unsafe_code)]

use blob_nix_nixos_system_workspace::NixOsSystemWorkspacePreview;

fn main() {
    print!(
        "{}",
        NixOsSystemWorkspacePreview::bluetooth_demo().proposed_canonical_system_spec
    );
}
