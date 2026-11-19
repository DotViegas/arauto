# Facts checked on the build of the connector

**Status: checked facts.** What was compared and what was not.

## Reproducibility of the connector build (checked 19/11/2026)

**Result: the two builds are not byte-identical.**

| Item | Value |
|---|---|
| Source | commit `0bba3eb` plus the working tree with the uncommitted changes of decision 0053 |
| Compiler | `rustc 1.98.1 (48a229cea 2026-09-01)` |
| Target | Windows x86_64, MSVC (the host target) |
| Command | `cargo build -j 1 --locked --release -p arauto-connector`, no `tls` feature |
| Path | the same both times: `CARGO_TARGET_DIR` at `.dev/repro` inside the repository, removed between the two builds |
| First build, SHA-256 | `3af53f55c7fa0893db1ee380a9f8044f911ad5371c485d90bb00f4d46a563178` |
| Second build, SHA-256 | `b9e19911a4e071914a13ec1d27afd991e818804cb003720dd1442d126461e4ff` |
| Equal | no |

The cause was not investigated. A likely one is a value the MSVC linker writes into the file
(such as a timestamp or a signature of the debug information); that is a guess, not a fact.

## What this does not prove

- The Linux image builds (with `tls`, in Docker), the Windows GNU build and the two macOS builds
  were not checked: the development machine has no Docker and no C compiler for those targets.
- The image check stays pending for the server: build the image twice from one commit and compare
  the four digests of the connectors.
- Nothing here says the Linux build is or is not reproducible.

## What follows from it

The design does not depend on reproducibility. The hub compares the files it serves with digests
made in the same image build (`docs/11-conector.md`, section 7), so the bundle is known after each
rebuild of the image. What a rebuild changes is the digest a device declared: a connector installed
from an earlier build of the same sources can show as "Other build" until that Hermes is
reconnected.
