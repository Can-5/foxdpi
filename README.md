# FoxDPI

Single-command universal package manager for Linux.

## Bash installer (curl-shroud style)

```bash
curl -sSf https://raw.githubusercontent.com/ilkaycan/foxdpi/main/install.sh | sh
```

This installs foxdpi for the current user (no sudo by default) into `~/.local/bin` and
creates a shell alias for convenience.  Use `--system` to install for all users (requires
sudo).

## Source / pip

```bash
git clone https://github.com/ilkaycan/foxdpi.git
cd foxdpi
pip install -e .
```

## Supported package formats

- `.deb` (dpkg/apt)
- `.rpm` (dnf/yum/rpm)
- `.tar.gz` / `.tar.bz2` / `.tar.zst` / `.zip` (archive extraction with PATH setup)
- AppImage (make executable + symlink)
- AUR / Arch build approach (optional helper that inspects PKGBUILD and uses makepkg/yay/paru when available)
- Custom remote-install pipeline (`foxdpi remote install <url>`) modeled after curl-sh installers

## Quick start

```bash
foxdpi install ./package.deb               # install local file using distro-aware strategy
foxdpi install https://example.com/pkg.rpm # remote install with checksum/GPG optional
foxdpi install appimage https://.../app.AppImage
foxdpi list                               # show installed foxdpi-managed packages
foxdpi remove <name>                      # remove a managed package
foxdpi doctor                            # report OS, package managers, and readiness
foxdpi run <binary> -- <args>            # run a foxdpi-managed binary (if any) through PATH
```
