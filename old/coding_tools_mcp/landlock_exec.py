from __future__ import annotations

import ctypes
import os
import shutil
import sys
from typing import Any


PR_SET_NO_NEW_PRIVS = 38
SYS_LANDLOCK_RESTRICT_SELF = 446

_LIBC: Any | None = None


def landlock_libc() -> Any:
    global _LIBC
    if _LIBC is None:
        _LIBC = ctypes.CDLL(None, use_errno=True)
    return _LIBC


def libc_syscall(number: int, *args: object) -> int:
    ctypes.set_errno(0)
    return int(landlock_libc().syscall(number, *args))


def fail(message: str) -> int:
    print(message, file=sys.stderr)
    return 126


def main(argv: list[str] | None = None) -> int:
    if sys.platform != "linux":
        return fail("landlock_exec is only supported on Linux")
    args = list(sys.argv[1:] if argv is None else argv)
    if len(args) != 2:
        return fail("landlock_exec requires: <ruleset-fd> <command>")
    try:
        ruleset_fd = int(args[0])
    except ValueError:
        return fail("landlock_exec received an invalid ruleset fd")
    cmd = args[1]

    rc = int(landlock_libc().prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0))
    if rc != 0:
        err = ctypes.get_errno()
        return fail(f"failed to set no_new_privs before Landlock restrict: {os.strerror(err)}")
    rc = libc_syscall(SYS_LANDLOCK_RESTRICT_SELF, ruleset_fd, 0)
    if rc != 0:
        err = ctypes.get_errno()
        return fail(f"failed to apply Landlock restrict_self: {os.strerror(err)}")
    try:
        os.close(ruleset_fd)
    except OSError:
        pass

    shell = os.environ.get("SHELL") or shutil.which("sh") or "/bin/sh"
    os.execvpe(shell, [shell, "-c", cmd], os.environ)
    return 127


if __name__ == "__main__":
    raise SystemExit(main())
