"""
FoxDPI core — global DPI (FortDPI / Global DPI) seviyesinde bypass motorunun
dahili tampon (buffer) ve packet-range yönetimi.

Bu katman, SOCKS5 proxy / bypass motoru tarafından kullanılan Buffer class'
ını import eder ve örnek kullanım demonstration gösterir.
"""

from __future__ import annotations

from foxdpi.buffer import Buffer

__all__ = ["Buffer"]


def example_usage() -> None:
    buf = Buffer()
    buf.write(b"hello ")
    buf.write(b"world")
    print("read(5):", buf.read(5))
    print("read():", buf.read())
    print("empty:", buf.empty())


if __name__ == "__main__":
    example_usage()
