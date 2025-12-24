<!--
This README file accompanies the ratadroid‑cli project.  It provides an overview of
the command‑line interface and step‑by‑step instructions for configuring a
development environment for Rust‑based terminal applications targeting Android.
The CLI aims to automate as much of the boilerplate as possible while still
allowing developers full control over their build pipeline.
-->

# Ratadroid CLI

`ratadroid‑cli` is a lightweight companion tool for developers building
terminal user interface (TUI) applications in Rust on Android.  It automates
common setup tasks, wraps `cargo‑ndk` to build your crate for Android
architectures and hosts generated APKs over HTTP for easy installation on
physical devices during development.

## Features

- **Guided environment setup**:  The `init` subcommand prints instructions
  and attempts to install `cargo‑ndk` and add common Android targets via
  `rustup`.  It also reminds you to install the Android NDK using Android
  Studio’s SDK Manager and set the `ANDROID_NDK_HOME` environment variable.
- **One‑line builds**:  Use `ratadroid‑cli build` to invoke `cargo ndk` with
  sensible defaults.  You can specify the target triple with
  `--target`, and the build artifacts are emitted into a `dist` directory.
- **HTTP file server**:  The `serve` subcommand spins up a small
  Hyper‑based server that exposes the contents of a directory (default `dist`) on
  your local network.  Browsing to `http://<host>:<port>/` lists the files
  and allows direct download of `.apk` packages on your phone or tablet.
- **Works with modern crates**:  The tool is designed for Rust 2024
  edition projects and plays nicely with recent mobile tooling such as
  [`android‑activity`](https://crates.io/crates/android‑activity) and the
  Robius ecosystem, which provide first‑class support for deep links and
  other Android platform features.
- **Environment diagnostics**:  A new `doctor` command examines your system for
  common Android and Rust prerequisites – it checks that the Android SDK and
  NDK are present and that licenses are accepted, verifies that `cargo‑ndk` and
  the necessary Rust targets are installed, and scans for available emulators.
  With `--fix` the doctor attempts to install missing components automatically
  using `cargo install`, `rustup target add` and `sdkmanager`.

## Installation

Clone or download this repository and build it with a recent Rust toolchain
(`rustup install stable` if you haven’t already).  Then run:

```sh
cd ratadroid_cli
cargo install --path .
```

This installs `ratadroid‑cli` into your `~/.cargo/bin` directory.  You can
also run it locally via `cargo run -- <subcommand>` while developing.

## Usage

Run `ratadroid‑cli --help` for an overview of available commands.  The
subcommands are described in detail below.

### Initialize your environment

```
ratadroid‑cli init
```

This command performs a best‑effort configuration of your Android build
environment:

1. **Install the NDK** – You must manually install the Android NDK via
   Android Studio’s SDK Manager.  Once installed, set the `ANDROID_NDK_HOME`
   environment variable to the NDK directory.
2. **Install `cargo‑ndk`** – The CLI attempts to run
   `cargo install cargo-ndk --force`.  If installation fails (for example,
   because `cargo` is not on your `PATH`), the tool prints a warning and
   instructs you to install it manually.
3. **Add Android targets** – The CLI runs `rustup target add` for the
   following architectures: `aarch64-linux-android`, `armv7-linux-androideabi`,
   `i686-linux-android` and `x86_64-linux-android`.  These targets are
   necessary to build Rust crates for modern devices and emulators.
4. **Optional Robius crates** – For advanced features like deep links and
   secure storage, add crates such as `robius-url-handler` and
   `robius-android-env` to your `Cargo.toml`.  The CLI does not install
   these automatically but reminds you of their existence.

### Build your project

```
ratadroid‑cli build [--target <triple>]
```

This command wraps `cargo ndk build --release` and emits artifacts into a
`dist` directory.  By default it targets `aarch64-linux-android`, which is
appropriate for modern 64‑bit ARM devices.  Specify `--target
armv7-linux-androideabi`, `i686-linux-android` or `x86_64-linux-android` to
build for other architectures.

Internally, the CLI spawns a child process to run `cargo ndk`.  If you
haven’t installed `cargo‑ndk` or set `ANDROID_NDK_HOME`, the build will
fail and you’ll see an error message.

### Serve built APKs

```
ratadroid‑cli serve [--port <number>] [--dir <path>]
```

Use this subcommand to start a small HTTP server for sharing your compiled
APKs and other files.  By default it serves the `dist` directory on port
8000.  The server prints the absolute path being served and the URL to
navigate to from your phone’s browser.  Any files in the directory (including
`.apk`) are listed on the index page.  Clicking a link downloads the file.

You can specify a different port or directory if needed.  For example:

```sh
ratadroid‑cli serve --port 9000 --dir path/to/apks
```

### Diagnose your environment

```
ratadroid‑cli doctor [--fix]
```

The `doctor` subcommand inspects your development machine and reports on
critical components:

1. **Rust prerequisites** – Checks whether `cargo‑ndk` is installed and
   whether the Android Rust targets (`aarch64-linux-android`,
   `armv7-linux-androideabi`, `i686-linux-android`, `x86_64-linux-android`) are
   available via `rustup`.  If any targets are missing, the doctor lists
   them and, when `--fix` is specified, runs `rustup target add` to
   install them.
2. **Android SDK and NDK** – Looks for the SDK by examining the
   `ANDROID_SDK_ROOT` or `ANDROID_HOME` environment variables and common
   default directories on Windows, macOS, and Linux.  If found, it
   verifies that the commandline tools (`sdkmanager`, `avdmanager`,
   `emulator`) exist and that an NDK is installed.  When the SDK is
   missing or incomplete, the doctor offers guidance and, with
   `--fix`, uses `sdkmanager` to install the NDK.
3. **AVD availability** – Invokes `emulator -list-avds` to show any
   existing Android Virtual Devices and warns if none are present.
4. **Licenses** – Checks whether the `licenses` directory within the
   SDK contains an `android-sdk-license` file.  If not, it recommends
   running `sdkmanager --licenses` to accept the terms.  With
   `--fix`, the doctor attempts to run this command automatically.

Run the doctor whenever you suspect your toolchain is misconfigured.  The
`--fix` flag automates remediation steps where possible, though manual
installation via Android Studio may still be necessary for certain
components (such as downloading the SDK itself).

## Running on real devices

1. Connect your Android device to the same Wi‑Fi network as your development
   machine.
2. Run `ratadroid‑cli serve` on your machine and note the IP address (e.g.
   `http://192.168.1.100:8000/`).
3. Open this URL in the mobile browser.  You should see a list of APKs.
4. Tap the APK you wish to install.  Make sure you have enabled
   *Install unknown apps* in Android settings.  For debugging builds you may
   need to allow installation from your browser.

## Contributing

The CLI is a simple utility intended to streamline development workflows.
Contributions, bug reports and feature requests are welcome.  Feel free to
open an issue or pull request on the project repository.

## License

This project is distributed under the MIT License.  See `LICENSE` for
details.