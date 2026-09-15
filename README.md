# Kaneo Desktop

Desktop client for Kaneo — https://github.com/usekaneo/kaneo

It opens a Kaneo instance — Kaneo Cloud or your own server — in its own
window, and keeps each instance's session, cookies and storage separate.

## Building

One codebase, three desktops. Requirements everywhere: **Node.js 24+**, **pnpm**,
and a **Rust stable toolchain** ([rustup](https://rustup.rs)), then:

```sh
pnpm install
pnpm tauri build
```

Bundles land in `src-tauri/target/release/bundle/`. `--debug` gives a faster,
unoptimised build; `--bundles app`, `--bundles dmg`, `--bundles nsis` … build a
single format. `pnpm check` runs the type checker, linter, formatter, clippy and
the Rust tests.

### macOS

Extra requirement: the Xcode Command Line Tools (`xcode-select --install`).
Builds on macOS 10.15+; per-instance webview storage needs **macOS 14+** (older
releases share one store).

```sh
pnpm tauri build          # .app and .dmg
```

Output: `bundle/macos/Kaneo Desktop.app` and
`bundle/dmg/Kaneo Desktop_<version>_<arch>.dmg`. Builds are ad-hoc signed by
default — see [Stable signing identity](#optional-stable-signing-identity).

### Windows

Requirements: Windows 10/11, the **Microsoft C++ Build Tools** ("Desktop
development with C++"), the **WebView2 runtime** (preinstalled on Windows 11; the
installer can bootstrap it otherwise), and the Rust target:

```powershell
rustup target add x86_64-pc-windows-msvc
pnpm tauri build
```

Output: `bundle/nsis/Kaneo Desktop_<version>_x64-setup.exe` and/or
`bundle/msi/Kaneo Desktop_<version>_x64_en-US.msi`.

- The `.msi` (WiX) **can only be built on Windows**. Cross-compiling the NSIS
  installer from macOS or Linux is possible with `cargo-xwin` plus LLVM and NSIS,
  but Tauri itself calls that a last resort — use a Windows machine or a CI runner.
- The installer downloads the WebView2 bootstrapper when the runtime is missing.
  `bundle.windows.webviewInstallMode` switches that to an embedded bootstrapper
  (~1.8 MB) or a fully offline installer (~127 MB).
- Unsigned installers show SmartScreen's "unknown publisher" prompt; an
  Authenticode certificate (or Azure Trusted Signing) clears it as reputation
  builds.

### Linux

Requirements (Debian/Ubuntu package names — adjust for your distribution):

```sh
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
pnpm install
pnpm tauri build
```

Output: `bundle/appimage/*.AppImage`, `bundle/deb/*.deb`, `bundle/rpm/*.rpm`. There
is no Gatekeeper equivalent; packages can be GPG-signed if you need it.

Two things worth knowing:

- The AppImage does **not** bundle WebKit — the app uses the distribution's
  WebKitGTK 4.1. Target a current distro (Ubuntu 24.04+, Fedora 40+ and similar):
  Kaneo's UI is built on Tailwind v4, which needs a modern WebKit.
- Cross-compiling from macOS is not practical. Build on Linux, in a container, or
  in CI.

### CI

The three platforms cannot all be built from one machine. A matrix of
`macos-latest`, `windows-latest` and `ubuntu-24.04` runners with
[`tauri-apps/tauri-action`](https://github.com/tauri-apps/tauri-action) produces
all three bundles and attaches them to a release.

## Install on macOS

Requires macOS 10.15 or newer. Per-instance cookie and storage isolation needs
macOS 14+; older releases fall back to a shared webview store.

### 1. Install

```sh
cp -R "src-tauri/target/release/bundle/macos/Kaneo Desktop.app" /Applications/
```

Or drag the app out of `src-tauri/target/release/bundle/macos/` in Finder.

### 2. First launch

The app is signed locally but not notarized by Apple, so the first launch of a
copy that arrived with the download quarantine flag is blocked with "Apple could
not verify …". Allow it once, by any of:

- Finder: right-click the app → **Open** → **Open**
- System Settings → **Privacy & Security** → **Open Anyway**
- `xattr -dr com.apple.quarantine "/Applications/Kaneo Desktop.app"`

An app you built on this Mac has no quarantine flag and launches directly.

### 3. Add an instance

Press **Use Kaneo Cloud**, or paste the URL of your own instance
(`https://kaneo.example.com`, `http://localhost:5173`, …) and press **Add
instance**. The URL is checked against the instance before it is stored, then
**Open** starts it in its own window. Sign in on the instance's own sign-in page;
the session stays inside that instance's window.

### The launcher window

The launcher is just a launcher — get it out of the way once your instance is
open:

- **Launcher → Hide Launcher** (⌘⇧H) hides the window; the app and your instance
  windows keep running.
- **Launcher → Show Launcher** (⌘⇧L) brings it back, and so does clicking the
  Dock icon.
- Closing the window hides it rather than quitting, so it can always be recalled.
  **Quit** (⌘Q) exits for real.
- Opening an instance hides the launcher for you, so the instance window gets the
  screen.

### Switching themes from the menu bar

The menu bar is the short path: **Themes** lists every theme with a tick on the
one in use, and **Themes → Theme Editor…** (⌘,) opens the editor. Picking a theme
restyles every open instance window straight away and survives a restart, whether
the launcher is open or not.

The launcher itself keeps the stock palette whatever is applied: the theme is for
the instances, and a fixed look here is what makes the app recognisable.

### Optional: stable signing identity

Builds are signed ad-hoc by default (`bundle.macOS.signingIdentity: "-"`). That
identity changes on every build, so macOS treats each rebuild as a different app
and re-asks for any permission you granted. A named, self-signed certificate
keeps the identity stable:

```sh
# once: create a self-signed code signing certificate and trust it.
# KANEO_SIGNING_PASSWORD is read from .env.local, which is gitignored.
set -a; . ./.env.local; set +a

mkdir -p ~/.kaneo-desktop/codesign && cd ~/.kaneo-desktop/codesign
openssl req -x509 -newkey rsa:2048 -sha256 -days 3650 -nodes \
  -keyout key.pem -out cert.pem \
  -subj "/CN=Kaneo Desktop Local/O=Kaneo Desktop/O=Local Development" \
  -addext "basicConstraints=critical,CA:false" \
  -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=critical,codeSigning"
# macOS' PKCS#12 importer needs the legacy algorithms here
openssl pkcs12 -export -inkey key.pem -in cert.pem -out identity.p12 \
  -name "Kaneo Desktop Local" -passout "pass:$KANEO_SIGNING_PASSWORD" \
  -certpbe PBE-SHA1-3DES -keypbe PBE-SHA1-3DES -macalg sha1
security import identity.p12 -k ~/Library/Keychains/login.keychain-db \
  -P "$KANEO_SIGNING_PASSWORD" -T /usr/bin/codesign -T /usr/bin/security
security add-trusted-cert -r trustRoot -p codeSign \
  -k ~/Library/Keychains/login.keychain-db cert.pem
security find-identity -v -p codesigning   # lists "Kaneo Desktop Local"

# each build
APPLE_SIGNING_IDENTITY="Kaneo Desktop Local" pnpm tauri build --bundles app
```

Copy `.env.example` to `.env.local` and set `KANEO_SIGNING_PASSWORD` there. That
file is gitignored, so the password never lands in the repository.

The certificate is trusted for code signing in your user keychain only. It does
not satisfy Gatekeeper on someone else's Mac — that needs an Apple Developer ID
and notarization.

### Uninstall

```sh
rm -rf "/Applications/Kaneo Desktop.app"
rm -rf "$HOME/Library/Application Support/app.kaneo.desktop"  # instance list
rm -rf "$HOME/Library/WebKit/app.kaneo.desktop"              # sessions, cookies, storage
rm -rf "$HOME/Library/Caches/app.kaneo.desktop"
```

## Themes

Kaneo Desktop restyles the instance's own interface — it does not reimplement it —
so a theme changes the app you already know. Twenty themes ship with it, and
each covers the mode or modes it defines: most carry **both** light and dark, so
switching Kaneo between appearances keeps the theme either way, while a
single-mode theme (Kanagawa Wave/Dragon, Kanagawa Lotus, Catppuccin
Latte/Frappe/Macchiato/Mocha) leaves the other mode on the instance's own
palette. The launcher window keeps its own fixed palette.

<img src="assets/theme-previews/grey-light.png" width="420" alt="Grey, light mode">

The previews below are the mockup the editor shows while you tweak a theme. The
editor is a window of its own: **Themes → Theme Editor…** (⌘,) opens it, and
**← Launcher** in its top-left corner takes you back. Switching themes does not
need it — **Themes** in the menu bar lists every theme and applies one on the
spot. That list is built when the app starts, so a theme you save in the editor
joins it on the next launch.

| theme | light | dark |
| --- | --- | --- |
| **Default**<br>Kaneo's own palette, untouched | <img src="assets/theme-previews/default-light.png" width="380" alt="Default light"> | <img src="assets/theme-previews/default-dark.png" width="380" alt="Default dark"> |
| **Grey**<br>cool grey-blue | <img src="assets/theme-previews/grey-light.png" width="380" alt="Grey light"> | <img src="assets/theme-previews/grey-dark.png" width="380" alt="Grey dark"> |
| **Dark Lilac**<br>lilac and violet | <img src="assets/theme-previews/dark-lilac-light.png" width="380" alt="Dark Lilac light"> | <img src="assets/theme-previews/dark-lilac-dark.png" width="380" alt="Dark Lilac dark"> |
| **Synthwave**<br>neon violet, pink and amber | <img src="assets/theme-previews/synthwave-light.png" width="380" alt="Synthwave light"> | <img src="assets/theme-previews/synthwave-dark.png" width="380" alt="Synthwave dark"> |
| **Aquamarine**<br>mint and deep teal | <img src="assets/theme-previews/aquamarine-light.png" width="380" alt="Aquamarine light"> | <img src="assets/theme-previews/aquamarine-dark.png" width="380" alt="Aquamarine dark"> |
| **Sunset**<br>plum, crimson and peach | <img src="assets/theme-previews/sunset-light.png" width="380" alt="Sunset light"> | <img src="assets/theme-previews/sunset-dark.png" width="380" alt="Sunset dark"> |
| **Summer**<br>corn silk, tea green and bronze | <img src="assets/theme-previews/summer-light.png" width="380" alt="Summer light"> | <img src="assets/theme-previews/summer-dark.png" width="380" alt="Summer dark"> |
| **Pastels**<br>thistle, baby pink and icy blue | <img src="assets/theme-previews/pastels-light.png" width="380" alt="Pastels light"> | <img src="assets/theme-previews/pastels-dark.png" width="380" alt="Pastels dark"> |
| **Metallic Sky**<br>steel, silver and platinum greys | <img src="assets/theme-previews/metallic-sky-light.png" width="380" alt="Metallic Sky light"> | <img src="assets/theme-previews/metallic-sky-dark.png" width="380" alt="Metallic Sky dark"> |
| **Oldschool**<br>cream paper, dust grey and spicy paprika | <img src="assets/theme-previews/oldschool-light.png" width="380" alt="Oldschool light"> | <img src="assets/theme-previews/oldschool-dark.png" width="380" alt="Oldschool dark"> |
| **80s Colors**<br>almond cream, lilac ash and prussian blue | <img src="assets/theme-previews/80s-colors-light.png" width="380" alt="80s Colors light"> | <img src="assets/theme-previews/80s-colors-dark.png" width="380" alt="80s Colors dark"> |
| **Kanagawa Wave**<br>fuji white on sumi ink — dark theme | dark theme — light mode keeps the instance's palette | <img src="assets/theme-previews/kanagawa-wave-dark.png" width="380" alt="Kanagawa Wave dark"> |
| **Kanagawa Dragon**<br>Kanagawa's late-night variant — dark theme | dark theme — light mode keeps the instance's palette | <img src="assets/theme-previews/kanagawa-dragon-dark.png" width="380" alt="Kanagawa Dragon dark"> |
| **Kanagawa Lotus**<br>cream lotus paper and violet ink — light theme | <img src="assets/theme-previews/kanagawa-lotus-light.png" width="380" alt="Kanagawa Lotus light"> | light theme — dark mode keeps the instance's palette |
| **Solarized**<br>Ethan Schoonover's solarized blues and creams | <img src="assets/theme-previews/solarized-light.png" width="380" alt="Solarized light"> | <img src="assets/theme-previews/solarized-dark.png" width="380" alt="Solarized dark"> |
| **Calm Seas**<br>pale sea air, teal and deep water blue | <img src="assets/theme-previews/calm-seas-light.png" width="380" alt="Calm Seas light"> | <img src="assets/theme-previews/calm-seas-dark.png" width="380" alt="Calm Seas dark"> |
| **Catppuccin Latte**<br>Catppuccin's light flavour — light theme | <img src="assets/theme-previews/catppuccin-latte-light.png" width="380" alt="Catppuccin Latte light"> | light theme — dark mode keeps the instance's palette |
| **Catppuccin Frappe**<br>Catppuccin's lightest dark flavour — dark theme | dark theme — light mode keeps the instance's palette | <img src="assets/theme-previews/catppuccin-frappe-dark.png" width="380" alt="Catppuccin Frappe dark"> |
| **Catppuccin Macchiato**<br>Catppuccin's mid flavour — dark theme | dark theme — light mode keeps the instance's palette | <img src="assets/theme-previews/catppuccin-macchiato-dark.png" width="380" alt="Catppuccin Macchiato dark"> |
| **Catppuccin Mocha**<br>Catppuccin's deepest flavour — dark theme | dark theme — light mode keeps the instance's palette | <img src="assets/theme-previews/catppuccin-mocha-dark.png" width="380" alt="Catppuccin Mocha dark"> |

Themes are plain YAML files listing 41 colour roles — see
[`themes/template.yaml`](themes/template.yaml) for the annotated template. The
editor writes your own to `~/Library/Application Support/app.kaneo.desktop/themes/`,
with the SVG preview, a contrast verdict per role and an auto-adjust that moves a
colour as little as it can to pass. Every built-in except Default clears WCAG AA
on text, icons, focus rings and input borders; the eight themes marked in their
files carry one measured exception where the palette cannot satisfy both the
non-text and text rules at once.

## License

MIT — see [LICENSE](LICENSE).
