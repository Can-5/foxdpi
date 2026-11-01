"""FoxDPI dahili tampon (buffer) katmanı.

SOCKS5 proxy veya bypass motoru, bağlantı üzerinden gelen baytları bu
Buffer'a yazar. listener'lar (proxy, engine, block-page katmanı) buffer'dan
okur. enable=False ise write no-op, emit yoktur.
"""

from __future__ import annotations

from collections import deque
from typing import Any, Callable

PayloadT = list[bytes]
ListenerT = Callable[[Any], None]


class Buffer:
    """baytları depolayan, enable/disable, listener (None gönderir) destekli tampon."""

    def __init__(self) -> None:
        self._buf: PayloadT = []
        self._listeners: list[ListenerT] = []
        self._enabled: bool = True

    def write(self, data: bytes) -> None:
        if not self._enabled:
            return
        self._buf.append(data)

    def read(self, size: int = -1) -> bytes:
        if size < 0:
            out: list[bytes] = list(self._buf)
            self._buf.clear()
            return b"".join(out)
        out: list[bytes] = []
        remaining = size
        while self._buf and remaining > 0:
            chunk = self._buf[0]
            if len(chunk) <= remaining:
                out.append(chunk)
                remaining -= len(chunk)
                self._buf.pop(0)
            else:
                out.append(chunk[:remaining])
                self._buf[0] = chunk[remaining:]
                remaining = 0
        return b"".join(out)

    def emit(self) -> None:
        for listener in self._listeners:
            listener(None)

    def add_listener(self, cb: ListenerT) -> None:
        if cb not in self._listeners:
            self._listeners.append(cb)

    def remove_listener(self, cb: ListenerT) -> None:
        if cb in self._listeners:
            self._listeners.remove(cb)

    def flush(self) -> None:
        self._buf.clear()
        for listener in self._listeners:
            listener(None)

    def enable(self, flag: bool) -> None:
        self._enabled = flag

    def disabled(self) -> bool:
        return not self._enabled

    def empty(self) -> bool:
        return len(self._buf) == 0

    def full(self) -> bool:
        return len(self._buf) > 0

    def get_packet_range(self, start: int = 0, end: int | None = None) -> list[bytes]:
        if end is None:
            end = len(self._buf)
        return list(self._buf[start:end])
