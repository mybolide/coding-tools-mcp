from __future__ import annotations

import sys


print("ready", flush=True)
for line in sys.stdin:
    value = line.rstrip("\n")
    if value == "exit":
        print("bye", flush=True)
        break
    print(f"echo:{value}", flush=True)
