# Docker Sandbox

The Docker image is a deployment shape for a toolchain-ready sandbox. It does not infer project type or run hidden install/build/test logic.

Build locally:

```bash
docker build -t coding-tools-mcp-sandbox:local .
```

The JDK major version is defined once as a build argument (`ARG JAVA_VERSION=17`); override it with `--build-arg JAVA_VERSION=21` when building.

Run against the current repository:

```bash
docker run --rm -it \
  -p 8765:8765 \
  -v "$PWD:/workspace" \
  coding-tools-mcp-sandbox:local
```

The container entrypoint is the `coding-tools-mcp` server itself; all configuration flows through `CODING_TOOLS_MCP_*` environment variables baked into the image:

```text
CODING_TOOLS_MCP_WORKSPACE=/workspace
CODING_TOOLS_MCP_HOST=0.0.0.0
CODING_TOOLS_MCP_PORT=8765
CODING_TOOLS_MCP_PERMISSION_MODE=trusted
CODING_TOOLS_MCP_GENERATE_AUTH_TOKEN=1
```

This is equivalent to starting `coding-tools-mcp --host 0.0.0.0 --port 8765 --permission-mode trusted` against `/workspace`. Override any of these with `docker run -e`.

Because the container binds to `0.0.0.0`, the server requires HTTP authentication. If you do not set `CODING_TOOLS_MCP_AUTH_TOKEN` or enable OAuth (`CODING_TOOLS_MCP_AUTH_MODE=oauth` or `CODING_TOOLS_MCP_OAUTH_MODE=1`), the server generates a bearer token at startup and prints it to stderr (`CODING_TOOLS_MCP_GENERATE_AUTH_TOKEN=1` opts the image into this). Set `CODING_TOOLS_MCP_AUTH_MODE=noauth` to disable generation, in which case a non-loopback bind without credentials fails fast.

The image also sets a default `CODING_TOOLS_MCP_EXEC_ALLOW_ROOTS` for container toolchain configuration files:

```text
/etc/java-17-openjdk:/etc/maven
```

(Everything under `/usr` — including the JVM and Maven installs — is already a built-in Landlock read root, and `JAVA_HOME` is added automatically.) Set `CODING_TOOLS_MCP_EXEC_ALLOW_ROOTS` yourself when you need to replace that default.

Use an explicit token when you need deterministic client configuration:

```bash
docker run --rm -it \
  -p 8765:8765 \
  -e CODING_TOOLS_MCP_AUTH_TOKEN="$(python3 -c 'import secrets; print(secrets.token_urlsafe(32))')" \
  -v "$PWD:/workspace" \
  coding-tools-mcp-sandbox:local
```

Dangerous mode must be explicit:

```bash
docker run --rm -it \
  -p 8765:8765 \
  -e CODING_TOOLS_MCP_PERMISSION_MODE=dangerous \
  -v "$PWD:/workspace" \
  coding-tools-mcp-sandbox:local
```

The server prints:

```text
WARNING: permission_mode=dangerous disables MCP safety gates. Use only inside an isolated container or VM.
```

Smoke commands should be explicit `exec_command` calls (CI runs them via `scripts/mcp_smoke.py`), for example:

```bash
java -version
javac -version
mvn -version
gcc --version
node --version && npm --version
python --version
go version
cargo --version && rustc --version
```
