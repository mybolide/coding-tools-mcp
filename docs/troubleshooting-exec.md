# Troubleshooting Exec Command

`exec_command` preserves raw `stdout`, `stderr`, and `exit_code`. It may also return `diagnostics` with common failure codes.

Common codes:

- `DEV_NULL_DENIED`: Landlock special device rules are wrong for `/dev/null`.
- `TMPDIR_NOT_WRITABLE`: the configured temp directory is not writable.
- `HOME_NOT_WRITABLE`: the configured home directory is not writable.
- `DNS_RESOLUTION_FAILED`: resolver configuration or network/DNS failed.
- `NETWORK_PERMISSION_REQUIRED`: safe mode blocked a network-looking command.
- `SHELL_EXPANSION_PERMISSION_REQUIRED`: safe mode blocked shell expansion.
- `INLINE_SCRIPT_PERMISSION_REQUIRED`: safe mode blocked inline interpreter or shell code.
- `LANDLOCK_READ_ROOT_BLOCKED`: a toolchain file path is missing from read roots.
- `SECRET_ENV_REJECTED`: secret-looking or loader/startup env was rejected.
- `COMMAND_TIMED_OUT`: the command exceeded `timeout_ms`.
- `OUTPUT_TRUNCATED`: stdout or stderr exceeded output limits.

Useful explicit probes:

```bash
dd if=/dev/null of=/dev/null bs=1 count=0
echo hi >/dev/null
printf ok > "$HOME/coding-tools-write-test"
printf ok > "$TMPDIR/coding-tools-write-test"
cat /etc/resolv.conf && getent hosts repo.maven.apache.org
```
