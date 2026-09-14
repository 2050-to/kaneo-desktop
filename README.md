# Kaneo Desktop

Desktop client for Kaneo — https://github.com/usekaneo/kaneo

It opens a Kaneo instance — Kaneo Cloud or your own server — in its own
window, and keeps each instance's session, cookies and storage separate.

## Install on macOS

Requires macOS 10.15 or newer. Per-instance cookie and storage isolation needs
macOS 14+; older releases fall back to a shared webview store.

### 1. Build

Build dependencies: Node.js 24+, pnpm, Rust stable, and the Xcode Command Line
Tools (`xcode-select --install`).

```sh
pnpm install
pnpm tauri build --bundles app     # --debug for a faster, unoptimised build
```

The result is `src-tauri/target/release/bundle/macos/Kaneo Desktop.app`
(`target/debug/...` after a `--debug` build).

### 2. Install

```sh
cp -R "src-tauri/target/release/bundle/macos/Kaneo Desktop.app" /Applications/
```

Or drag the app out of `src-tauri/target/release/bundle/macos/` in Finder.

### 3. First launch

The app is signed locally but not notarized by Apple, so the first launch of a
copy that arrived with the download quarantine flag is blocked with "Apple could
not verify …". Allow it once, by any of:

- Finder: right-click the app → **Open** → **Open**
- System Settings → **Privacy & Security** → **Open Anyway**
- `xattr -dr com.apple.quarantine "/Applications/Kaneo Desktop.app"`

An app you built on this Mac has no quarantine flag and launches directly.

### 4. Add an instance

Press **Use Kaneo Cloud**, or paste the URL of your own instance
(`https://kaneo.example.com`, `http://localhost:5173`, …) and press **Add
instance**. The URL is checked against the instance before it is stored, then
**Open** starts it in its own window. Sign in on the instance's own sign-in page;
the session stays inside that instance's window.

### Optional: stable signing identity

Builds are signed ad-hoc by default (`bundle.macOS.signingIdentity: "-"`). That
identity changes on every build, so macOS treats each rebuild as a different app
and re-asks for any permission you granted. A named, self-signed certificate
keeps the identity stable:

```sh
# once: create a self-signed code signing certificate and trust it
mkdir -p ~/.kaneo-desktop/codesign && cd ~/.kaneo-desktop/codesign
openssl req -x509 -newkey rsa:2048 -sha256 -days 3650 -nodes \
  -keyout key.pem -out cert.pem \
  -subj "/CN=Kaneo Desktop Local/O=Kaneo Desktop/O=Local Development" \
  -addext "basicConstraints=critical,CA:false" \
  -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=critical,codeSigning"
# macOS' PKCS#12 importer needs the legacy algorithms here
openssl pkcs12 -export -inkey key.pem -in cert.pem -out identity.p12 \
  -name "Kaneo Desktop Local" -passout pass:kaneo-desktop-local \
  -certpbe PBE-SHA1-3DES -keypbe PBE-SHA1-3DES -macalg sha1
security import identity.p12 -k ~/Library/Keychains/login.keychain-db \
  -P kaneo-desktop-local -T /usr/bin/codesign -T /usr/bin/security
security add-trusted-cert -r trustRoot -p codeSign \
  -k ~/Library/Keychains/login.keychain-db cert.pem
security find-identity -v -p codesigning   # lists "Kaneo Desktop Local"

# each build
APPLE_SIGNING_IDENTITY="Kaneo Desktop Local" pnpm tauri build --bundles app
```

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
