"""FoxDPI installer registry tascoplama — her installer modülü yüklenince register'den bağlanır."""

from __future__ import annotations

from foxdpi.installers import register  # noqa: F401
from foxdpi.installers.deb import DebInstaller  # noqa: F401
from foxdpi.installers.rpm import RpmInstaller  # noqa: F401
from foxdpi.installers.tarball import TarInstaller  # noqa: F401
from foxdpi.installers.appimage import AppImageInstaller  # noqa: F401
from foxdpi.installers.remote import RemoteInstaller  # noqa: F401
from foxdpi.installers.arch import ArchInstaller  # noqa: F401
from foxdpi.installers.script import ScriptInstaller  # noqa: F401
