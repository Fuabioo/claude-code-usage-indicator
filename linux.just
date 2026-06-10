# Linux / Pop!_OS COSMIC applet recipes.
# Imported by the root justfile; recipes are gated to [linux].

name := 'cosmic-applet-cc-usage'
appid := 'dev.fuabioo.CosmicAppletCcUsage'
res := 'crates/cosmic-applet-cc-usage/resources'

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

# Build the COSMIC applet (release).
[linux]
build-linux:
    cargo build --release -p {{name}}

# Install to user-local directories (no sudo).
[linux]
install-linux: build-linux
    install -Dm0755 target/release/{{name}} {{user-bindir}}/{{name}}
    install -Dm0644 {{res}}/{{appid}}.desktop {{user-appdir}}/{{appid}}.desktop
    install -Dm0644 {{res}}/{{appid}}.metainfo.xml {{user-metainfodir}}/{{appid}}.metainfo.xml
    install -Dm0644 {{res}}/icons/{{appid}}.svg {{user-iconsdir}}/{{appid}}.svg

# Install to system directories (requires sudo).
[linux]
install-system-linux: build-linux
    install -Dm0755 target/release/{{name}} {{bindir}}/{{name}}
    install -Dm0644 {{res}}/{{appid}}.desktop {{appdir}}/{{appid}}.desktop
    install -Dm0644 {{res}}/{{appid}}.metainfo.xml {{metainfodir}}/{{appid}}.metainfo.xml
    install -Dm0644 {{res}}/icons/{{appid}}.svg {{iconsdir}}/{{appid}}.svg

# Uninstall from user-local directories.
[linux]
uninstall-linux:
    rm -f {{user-bindir}}/{{name}}
    rm -f {{user-appdir}}/{{appid}}.desktop
    rm -f {{user-metainfodir}}/{{appid}}.metainfo.xml
    rm -f {{user-iconsdir}}/{{appid}}.svg

# Uninstall from system directories.
[linux]
uninstall-system-linux:
    rm -f {{bindir}}/{{name}}
    rm -f {{appdir}}/{{appid}}.desktop
    rm -f {{metainfodir}}/{{appid}}.metainfo.xml
    rm -f {{iconsdir}}/{{appid}}.svg
