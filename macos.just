# macOS menu bar app recipes.
# Imported by the root justfile; recipes are gated to [macos].

mac-app := 'CcUsageMenuBar'
swift-dir := 'macos/CcUsageMenuBar'
bundle := 'target/macos/' + mac-app + '.app'

# Build the Swift menu bar app (release). The CLI is built by the shared `build-cli` recipe.
[macos]
build-macos: build-cli
    cd {{swift-dir}} && swift build -c release

# Assemble a self-contained .app bundle (Swift binary + Info.plist + bundled cc-usage CLI).
[macos]
bundle-macos: build-macos
    rm -rf {{bundle}}
    mkdir -p {{bundle}}/Contents/MacOS {{bundle}}/Contents/Resources
    cp {{swift-dir}}/.build/release/{{mac-app}} {{bundle}}/Contents/MacOS/{{mac-app}}
    cp {{swift-dir}}/Resources/Info.plist {{bundle}}/Contents/Info.plist
    cp target/release/cc-usage {{bundle}}/Contents/Resources/cc-usage
    codesign --force --deep --sign - {{bundle}} || echo "warning: ad-hoc codesign failed (continuing)"
    @echo "Built {{bundle}}"

# Install the .app into ~/Applications.
[macos]
install-macos: bundle-macos
    rm -rf {{env('HOME')}}/Applications/{{mac-app}}.app
    cp -R {{bundle}} {{env('HOME')}}/Applications/{{mac-app}}.app
    @echo "Installed to ~/Applications/{{mac-app}}.app"

# Build, bundle, and launch the app.
[macos]
run-macos: bundle-macos
    open {{bundle}}

# Run the CLI directly, e.g. `just cli --status` or `just cli --json` (no leading `--`).
[macos]
cli *ARGS:
    cargo run -q -p cc-usage-cli -- {{ARGS}}
