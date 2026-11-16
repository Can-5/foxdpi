"""FoxDPI – universal Linux package manager / service helper CLI."""

try:
    from importlib.metadata import version as _version
    __version__ = _version("foxdpi")
except Exception:
    __version__ = "0.0.0+dev"
