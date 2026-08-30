# Blobsh v0.1

Status: architectural contract + P0 shell demo integration.

## Product statement

Blobsh is an AI-native, inspectable shell that can be embedded in The Blob OS or installed as a standalone tool on an existing operating system.

The Blob OS is the environment where Blobsh can integrate most deeply with Workspace, Surface, Fabric, SystemSpec, capability resolution, authority and provenance. Blobsh itself must not depend on the Blob characters, the Slint renderer, NixOS, or a particular desktop environment.

The core interaction principle is:

> Ask above. Inspect and control below.

The AI remains available at every level of expertise. Expert mode means greater precision and visibility, not less assistance.

## Dual-bar interaction

The primary shell surface contains two independently collapsible bars.

```text
λ AI     natural-language request
> BLOB   canonical command / currently inspected representation
```

Submitting the AI bar does not directly execute the requested operation. It prepares an inspectable command trace. The user can review or edit the BLOB representation before processing it.

Direct expert entry into the BLOB bar is a first-class interaction, not a fallback.

## Depth Inspector

A command may expose these levels when they really exist:

```text
USER
  ↓
INTENT
  ↓
PLAN
  ↓
SPEC
  ↓
BACKEND
  ↓
NATIVE
```

The UI must never invent a deeper representation simply to make the trace look complete. A Workspace-only intent may stop at INTENT. The current Bluetooth P0 reaches a semantic SPEC/proposal and deliberately stops before BACKEND/NATIVE because no corresponding materialization has been produced at that boundary yet.

### USER

The human request as entered.

Example:

```text
attiva bluetooth
```

### INTENT

Canonical Blobsh command.

```text
system.bluetooth enable
```

This is normally the first precise editable representation proposed by AI.

### PLAN

Resolved target, capabilities, constraints, authority requirements and intended operation sequence.

### SPEC

Declarative semantic desired state such as SystemSpec changes.

### BACKEND

Platform-specific materialization plan such as NixOS options, systemd operations, launchd configuration, package-manager operations or API calls.

### NATIVE

The lowest concrete operation representation available for the host. It may be a shell command, but it may also be D-Bus, an API operation, a filesystem transaction, an RPC, or another native mechanism.

Blobsh must not pretend that every semantic operation maps to one Bash command.

## Editing semantics

Every materialized level may declare whether it is editable.

Editing an upper layer invalidates every derived layer below it:

```text
AI proposed INTENT
       ↓
user edits INTENT
       ↓
old PLAN/SPEC/BACKEND/NATIVE become invalid
       ↓
re-validation and regeneration required
```

Editing NATIVE is different. It is preserved as an explicit `UserNativeOverride` and must not be retroactively presented as though it had been derived from the original intent.

Provenance is part of the model, not presentation metadata.

Current provenance categories include:

- UserInput
- AiProposed
- Derived
- BackendGenerated
- UserEdited
- UserNativeOverride

## AI assistance at every depth

Blobsh must not implement the pattern:

```text
beginner = AI
expert   = no AI
```

Instead:

```text
more depth
   ↓
more technical AI assistance
```

At any editable level the user should eventually be able to:

- request completions valid for that grammar;
- inspect available options/flags/fields;
- read a concise explanation;
- open authoritative documentation;
- ask AI why an option was selected in this machine/project context;
- ask AI to modify the selected command or field in natural language;
- compare the AI proposal with a user edit;
- keep an override even when AI recommends another choice.

## Assistance modes

The core reserves three modes:

```text
Quiet   completion only
Assist  completion + concise contextual guidance
Teach   explanations + alternatives + documentation + rationale
```

These modes affect assistance density, not user authority.

## Documentation model

Explanations should distinguish authoritative source material from AI contextual advice.

Potential sources include:

- Blob schema documentation;
- project documentation;
- platform documentation;
- man pages;
- NixOS options;
- ArchWiki or other selected reference sources;
- tool documentation such as Cargo, Git, Docker or systemd.

The UI should be able to show both:

```text
FACT / DOCUMENTATION
```

and

```text
AI CONTEXTUAL ADVICE
```

without conflating them.

## Execution and authority

Blobsh core does not execute commands. It models the trace, editing, assistance context and provenance.

Execution remains behind capability, validation, policy and authority boundaries.

A future UI may offer distinct actions such as:

```text
Preview native
Send to Terminal
Execute
```

Sending a command to a Terminal Surface should not imply automatic execution.

## Standalone architecture

Target architecture:

```text
blobsh-core
  ├── command trace / Depth Inspector
  ├── editing + invalidation
  ├── provenance
  ├── completion model
  ├── documentation references
  └── assistance mode

blobsh-runtime
  ├── intent parsing / translation
  ├── capability resolution
  ├── validation
  ├── policy / authority integration
  └── execution handoff

platform adapters
  ├── Blob OS
  ├── generic Linux
  └── macOS

frontends
  ├── The Blob Shell GUI
  ├── CLI/TUI
  └── desktop overlay
```

Only `blobsh-core` is introduced as a dedicated crate in P0. Additional crates should be extracted only when real implementation pressure makes their boundaries clear.

## Host strategy

### The Blob OS

Native integration with Workspace, Surface, Fabric, SystemSpec and The Blob authority model.

### Linux standalone

Adapters may integrate with the detected host environment, including tools such as shell, systemd, D-Bus, package managers, Nix, container runtimes and project toolchains. Availability must be discovered rather than assumed.

### macOS standalone

Adapters may integrate with shell, launchd, Homebrew and native macOS mechanisms. Again, availability is discovered and capabilities remain explicit.

Standalone Blobsh is a guest of the host OS. It does not pretend to own the whole machine model.

## Frontend strategy

The core experience must work without Blob characters.

Possible frontends:

```text
blobsh              CLI/TUI inside an existing terminal
blobsh-overlay      Spotlight-like desktop overlay
The Blob Shell      full Workspace/Blob graphical environment
```

The characters and World view are a distinctive The Blob experience, not a dependency of the standalone shell product.

## P0 demo behavior

The Slint demo now uses the dual-bar model.

Examples:

```text
λ AI     attiva bluetooth
> BLOB   system.bluetooth enable        [2/6 INTENT] [preview]
```

The BLOB command may be edited before processing.

After processing the Bluetooth demo, the real trace can expose:

```text
USER
INTENT
PLAN
SPEC
```

It deliberately does not fabricate BACKEND or NATIVE output.

Workspace navigation commands currently remain at INTENT because they are Shell semantic state transitions rather than host-native operations.

## Non-goals for v0.1

- replacing Bash/Zsh/Fish;
- executing arbitrary AI-generated shell text directly;
- pretending all actions resolve to terminal commands;
- autonomous privilege escalation;
- hiding platform operations from expert users;
- implementing Linux/macOS adapters before the common shell contract is stable.

## Product relationship

Long-term:

```text
The Blob OS
  complete AI-native operating environment

Blobsh
  installable AI-native inspectable shell for the computer you already have
```

Blobsh can therefore become an adoption path into The Blob philosophy without requiring users to replace their operating system.
