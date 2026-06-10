# Homebrew formula template for the macOS menu bar app + CLI.
#
# Copy this into your tap repo (e.g. homebrew-tap/Formula/cc-usage-menubar.rb), then set
# `url` to a release tarball and fill in `sha256` (`brew fetch` / `shasum -a 256` will print it).
#
# Builds from source on the user's machine, so it needs NO `just` and NO Apple notarization
# (the app is compiled locally, not downloaded prebuilt). Requires the Rust toolchain (pulled
# in as a build dep) and the Swift toolchain from the Xcode Command Line Tools (`xcode-select
# --install`), which Homebrew assumes on macOS.
class CcUsageMenubar < Formula
  desc "Claude Code usage budget — macOS menu bar app and CLI"
  homepage "https://github.com/Fuabioo/claude-code-usage-indicator"
  url "https://github.com/Fuabioo/claude-code-usage-indicator/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "REPLACE_WITH_TARBALL_SHA256"
  license "MIT"

  depends_on "rust" => :build
  depends_on :macos

  def install
    # 1. Cross-platform CLI.
    system "cargo", "build", "--release", "--locked", "-p", "cc-usage-cli"
    bin.install "target/release/cc-usage"

    # 2. Swift menu bar app.
    system "swift", "build", "-c", "release", "--package-path", "macos/CcUsageMenuBar"

    # 3. Assemble a self-contained .app (Swift binary + Info.plist + bundled CLI), then
    #    ad-hoc sign it (fine for a locally-built app).
    app = prefix/"CcUsageMenuBar.app"
    (app/"Contents/MacOS").mkpath
    (app/"Contents/Resources").mkpath
    cp "macos/CcUsageMenuBar/.build/release/CcUsageMenuBar", app/"Contents/MacOS/CcUsageMenuBar"
    cp "macos/CcUsageMenuBar/Resources/Info.plist", app/"Contents/Info.plist"
    cp bin/"cc-usage", app/"Contents/Resources/cc-usage"
    system "/usr/bin/codesign", "--force", "--deep", "--sign", "-", app
  end

  # `brew services start cc-usage-menubar` installs a per-user LaunchAgent that starts the
  # app at login. (Alternatively, use the app's own "Launch at Login" menu toggle.)
  service do
    run [opt_prefix/"CcUsageMenuBar.app/Contents/MacOS/CcUsageMenuBar"]
    keep_alive true
    run_type :immediate
  end

  def caveats
    <<~EOS
      The menu bar app was installed to:
        #{opt_prefix}/CcUsageMenuBar.app
      Open it once (or run `brew services start cc-usage-menubar`) to add the menu bar item.

      Credentials: on macOS the OAuth token lives in the login Keychain; the app reads it on
      first launch and you'll see a Keychain prompt — click "Always Allow".
    EOS
  end

  test do
    assert_match "cc-usage", shell_output("#{bin}/cc-usage --help")
  end
end
