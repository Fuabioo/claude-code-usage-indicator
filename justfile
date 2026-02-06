name := 'cosmic-applet-cc-usage'
appid := 'dev.fuabioo.CosmicAppletCcUsage'

# System install (requires sudo)
prefix := '/usr'
bindir := prefix / 'bin'
appdir := prefix / 'share' / 'applications'
iconsdir := prefix / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps'
metainfodir := prefix / 'share' / 'metainfo'

# User install (no sudo needed)
user-prefix := env('HOME') / '.local'
user-bindir := user-prefix / 'bin'
user-appdir := user-prefix / 'share' / 'applications'
user-iconsdir := user-prefix / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps'
user-metainfodir := user-prefix / 'share' / 'metainfo'

default: build-release

# Build debug binary
build-debug:
    cargo build

# Build release binary
build-release:
    cargo build --release

# Install to user-local directories (no sudo)
install: build-release
    install -Dm0755 target/release/{{name}} {{user-bindir}}/{{name}}
    install -Dm0644 resources/{{appid}}.desktop {{user-appdir}}/{{appid}}.desktop
    install -Dm0644 resources/{{appid}}.metainfo.xml {{user-metainfodir}}/{{appid}}.metainfo.xml
    install -Dm0644 resources/icons/{{appid}}.svg {{user-iconsdir}}/{{appid}}.svg

# Install to system directories (requires sudo)
install-system:
    install -Dm0755 target/release/{{name}} {{bindir}}/{{name}}
    install -Dm0644 resources/{{appid}}.desktop {{appdir}}/{{appid}}.desktop
    install -Dm0644 resources/{{appid}}.metainfo.xml {{metainfodir}}/{{appid}}.metainfo.xml
    install -Dm0644 resources/icons/{{appid}}.svg {{iconsdir}}/{{appid}}.svg

# Uninstall from user-local directories
uninstall:
    rm -f {{user-bindir}}/{{name}}
    rm -f {{user-appdir}}/{{appid}}.desktop
    rm -f {{user-metainfodir}}/{{appid}}.metainfo.xml
    rm -f {{user-iconsdir}}/{{appid}}.svg

# Uninstall from system directories
uninstall-system:
    rm -f {{bindir}}/{{name}}
    rm -f {{appdir}}/{{appid}}.desktop
    rm -f {{metainfodir}}/{{appid}}.metainfo.xml
    rm -f {{iconsdir}}/{{appid}}.svg

# Clean build artifacts
clean:
    cargo clean
