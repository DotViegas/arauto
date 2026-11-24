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

(24/11/2026: the Linux and Windows GNU files of the image were checked since; see "The
connectors of the image" below.)

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

## The connectors of the image (checked 24/11/2026)

**Result: the Linux file is reproducible. The Windows GNU file was not, because of the time of the
link, and is reproducible with `--no-insert-timestamp`.**

- Two builds of the image on the server, from commits `e0b146f` and `7350b2e` (the second changes
  only the hub): the Linux x86_64 connector had the same SHA-256 in both images; the Windows
  x86_64 GNU connector had two different digests. A Windows connector installed from the first
  image showed as "Other build" after the second one was deployed.
- In the build image (`rust:1.98-bookworm`, `gcc-mingw-w64-x86-64` of Debian bookworm), source
  `7350b2e`, the Windows connector linked twice, 34 seconds apart (only the connector crate built
  again): 3 bytes differ, two in the TimeDateStamp of the PE file header and one in the CheckSum of
  the optional header. `objdump -p` shows the two link times.
- The same two links with `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS='-C
  link-arg=-Wl,--no-insert-timestamp'`: byte-identical files, TimeDateStamp zero, no debug
  directory. The Dockerfile now builds the Windows connector this way and fails the build when the
  field is not zero.
- A build made by hand in a container, with the source on a bind mount, differed from the file of
  the image in the symbol table after the sections (about 372,000 bytes), with the same sections
  and the same code. Not investigated.

- The image of commit `90911e1`, the first with the option, built on the server on 24/11/2026:
  Windows file with TimeDateStamp zero; the Linux file and the two macOS files with the same
  digests as in the image of `7350b2e`. The macOS layer was built again (the Windows layer before
  it changed), so the two macOS files came out byte-identical from two builds.

Not checked:

- That two builds of the image with the option give the same Windows digest: expected, to be seen
  on the next deploy.
- No macOS file was ever run.

What follows: with the option, a rebuild of the image from sources that do not change the
connector keeps the digests it serves, and a connector installed from an earlier build stays
"Current". The first image built with the option serves a Windows file with a new digest: a
Windows connector installed before it shows as "Other build" once, until its file is replaced by
the one served or the Hermes reconnects.
