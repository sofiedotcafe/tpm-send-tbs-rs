<!-- markdownlint-disable MD033 MD013 -->
# tpm-send-tbs-rs

<a href="https://builtwithnix.org"><img src="https://builtwithnix.org/badge.svg" alt="Built with Nix" height="20"/></a>
<a href="https://learn.microsoft.com/en-us/windows/wsl/about"><img src="https://img.shields.io/badge/-WSL-%23c9d1d9?logo=linux&logoColor=black" alt="WSL" height="20"/></a>
<a href="https://github.com/RedHatPride/open-source-transition-resources"><img src="https://pride-badges.pony.workers.dev/static/v1?&stripeWidth=6&labelColor=%23c9d1d9&stripeColors=5BCEFA,F5A9B8,FFFFFF,F5A9B8,5BCEFA" alt="Pride Badge" height="20"/></a>

This project is a Rust fork of [tpm2-send-tbs](https://github.com/tpm2-software/tpm2-send-tbs).

It is rewritten in Rust to fix the known limitations of [tpm2-send-tbs](https://github.com/tpm2-software/tpm2-send-tbs), and it works as a bridge for WSL (Windows Subsystem for Linux) environments. It pipes raw TPM 2.0 command bytes from WSL into the Windows TPM Base Services (TBS) API, allowing Linux-based `tpm2-tools` to interact directly with the host Windows TPM.

---

## Requirements

* Windows 10/11 or Windows Server with an enabled TPM 2.0 module, running WSL.
* `tpm2-tools` installed inside WSL (or natively on Windows).

Pre-compiled binaries are automatically built using a Nix-based CI and are available directly in the **GitHub Releases** section.

---

## Usage

Run your WSL `tpm2-tools` commands by pointing the TCTI flag to the compiled Windows binary (`.exe`):

```bash
# Get random bytes from the Windows host TPM inside WSL
tpm2_getrandom -T "cmd:tpm2-send-tbs.exe" 16

# Read a host PCR register from WSL
tpm2_pcrread -T "cmd:tpm2-send-tbs.exe" sha256:7
```

### As standalone

You can also use it manually for piping and scripting:

```text
Usage: tpm2-send-tbs [OPTIONS]

Options:
  -i, --input <INPUT>    Input file path (defaults to stdin)
  -o, --output <OUTPUT>  Output file path (defaults to stdout)
      --hex              Format the output response into an ASCII hexadecimal string
      --bin              Force binary processing mode
  -v                     Increase logging verbosity (-v = info, -v = debug, -vv = trace)
  -h, --help             Print help
```

---

## Building & Cross-Compilation

To cross-compile from a Linux/WSL environment using Nix, run:

```bash
NIXPKGS_ALLOW_UNSUPPORTED_SYSTEM=1 nix build .#default --impure
```

If developing inside a native `devShell`, make sure your toolchain includes the `x86_64-pc-windows-gnu` target, then run:

```bash
cargo build --target x86_64-pc-windows-gnu --release
```

---

## Code Of Conduct

This project follows the [Lix Code of Conduct/Community Standards](https://lix.systems).

## License

This repository is distributed under the GNU General Public License v3 (GPLv3) or later.

GPLv3-or-later © 2026
