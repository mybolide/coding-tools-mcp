from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from pathlib import Path

import psutil

from .models import RuntimeStatus, WorkspaceProfile
from .storage import log_dir_for_profile, runtime_state_file_for_profile

TRYCLOUDFLARE_URL_RE = re.compile(r"https://[a-z0-9-]+\.trycloudflare\.com", re.I)


@dataclass
class ManagedSession:
    runtime_process: subprocess.Popen[str] | None = None
    tunnel_process: subprocess.Popen[str] | None = None
    runtime_pid: int | None = None


class RuntimeManager:
    RUNTIME_START_TIMEOUT_SECONDS = 20.0
    PORT_RELEASE_TIMEOUT_SECONDS = 8.0

    def __init__(self) -> None:
        self._sessions: dict[str, ManagedSession] = {}

    def start(self, profile: WorkspaceProfile) -> RuntimeStatus:
        try:
            self._validate_tunnel_requirements(profile)
        except Exception as exc:
            self._append_tunnel_error_log(profile, str(exc))
            raise
        self._cleanup_orphan_tunnel(profile)
        existing_pid = self._find_runtime_pid(profile)
        if existing_pid is not None:
            if profile.tunnel.type == "cloudflare" and self._find_tunnel_pid(profile) is None:
                try:
                    tunnel_process, public_url = self._start_cloudflare_tunnel(profile)
                except Exception as exc:
                    self._append_tunnel_error_log(profile, str(exc))
                    raise
                self._sessions[profile.id] = ManagedSession(
                    runtime_process=None,
                    tunnel_process=tunnel_process,
                    runtime_pid=existing_pid,
                )
                self._write_runtime_state(
                    profile,
                    runtime_pid=existing_pid,
                    tunnel_pid=tunnel_process.pid,
                    public_url=public_url,
                )
            return self.status(profile)

        conflicting_pid = self._find_pid_by_port(profile.runtime.local_port)
        if conflicting_pid is not None:
            conflict_command = self._command_line_for_pid(conflicting_pid)
            raise RuntimeError(
                "本地端口已被占用，无法启动。\n"
                f"端口：{profile.runtime.local_port}\n"
                f"占用进程 PID：{conflicting_pid}\n"
                f"命令行：{conflict_command or '未知'}"
            )

        runtime_process, runtime_pid = self._start_runtime_process(profile)
        tunnel_process: subprocess.Popen[str] | None = None
        public_url = self._server_url_for_profile(profile) if profile.tunnel.type == "frp" else ""

        try:
            if profile.tunnel.type == "cloudflare":
                tunnel_process, public_url = self._start_cloudflare_tunnel(profile)
            elif profile.tunnel.type != "frp":
                raise RuntimeError("当前仅支持 FRP 和 Cloudflare。")
        except Exception as exc:
            self._append_tunnel_error_log(profile, str(exc))
            self._terminate_process_tree(runtime_pid)
            if runtime_process.pid != runtime_pid:
                self._terminate_process_tree(runtime_process.pid)
            self._clear_runtime_state(profile.id)
            raise

        self._sessions[profile.id] = ManagedSession(
            runtime_process=runtime_process,
            tunnel_process=tunnel_process,
            runtime_pid=runtime_pid,
        )
        self._write_runtime_state(
            profile,
            runtime_pid=runtime_pid,
            tunnel_pid=tunnel_process.pid if tunnel_process else None,
            public_url=public_url,
        )
        return self.status(profile)

    def stop(self, profile: WorkspaceProfile) -> RuntimeStatus:
        session = self._sessions.pop(profile.id, None)
        state = self._read_runtime_state(profile.id)

        tunnel_pid: int | None = None
        runtime_pid: int | None = None
        if session is not None:
            if session.tunnel_process and session.tunnel_process.poll() is None:
                tunnel_pid = session.tunnel_process.pid
            if session.runtime_pid is not None:
                runtime_pid = session.runtime_pid
            elif session.runtime_process and session.runtime_process.poll() is None:
                runtime_pid = session.runtime_process.pid
        if tunnel_pid is None and isinstance(state.get("tunnel_pid"), int):
            tunnel_pid = state["tunnel_pid"]
        if runtime_pid is None and isinstance(state.get("runtime_pid"), int):
            runtime_pid = state["runtime_pid"]
        if runtime_pid is None and isinstance(state.get("pid"), int):
            runtime_pid = state["pid"]
        if tunnel_pid is None:
            tunnel_pid = self._find_tunnel_pid(profile)
        if runtime_pid is None:
            runtime_pid = self._find_runtime_pid(profile)

        if tunnel_pid is not None:
            self._terminate_process_tree(tunnel_pid)
        if runtime_pid is not None:
            self._terminate_process_tree(runtime_pid)
        if session is not None and session.runtime_process and session.runtime_process.poll() is None:
            if runtime_pid != session.runtime_process.pid:
                self._terminate_process_tree(session.runtime_process.pid)
        self._wait_for_port_state(profile.runtime.local_port, listening=False, timeout=self.PORT_RELEASE_TIMEOUT_SECONDS)

        self._clear_runtime_state(profile.id)
        return RuntimeStatus(state="stopped", local_message="已停止", public_message="已停止")

    def status(self, profile: WorkspaceProfile) -> RuntimeStatus:
        runtime_pid = self._find_runtime_pid(profile)
        tunnel_pid = self._find_tunnel_pid(profile)
        public_url = self.resolved_public_url(profile)

        if runtime_pid is None:
            if tunnel_pid is not None:
                self._terminate_process_tree(tunnel_pid)
            self._clear_runtime_state(profile.id)
            return RuntimeStatus(state="stopped", local_message="当前未运行", public_message="未知")

        if profile.tunnel.type == "cloudflare" and tunnel_pid is None:
            return RuntimeStatus(
                state="error",
                pid=runtime_pid,
                local_message=f"本地 MCP 正在监听 127.0.0.1:{profile.runtime.local_port}",
                public_message="Cloudflare 隧道未建立",
            )

        public_message = public_url or profile.endpoint
        if profile.tunnel.type == "cloudflare" and not public_url:
            public_message = "等待 Cloudflare 分配公网地址"
        return RuntimeStatus(
            state="running",
            pid=runtime_pid,
            local_message=f"正在监听 127.0.0.1:{profile.runtime.local_port}",
            public_message=public_message,
        )

    def summary_state(self, profile: WorkspaceProfile) -> str:
        return self.status(profile).state

    def resolved_public_url(self, profile: WorkspaceProfile) -> str:
        if profile.tunnel.type == "frp":
            return profile.effective_public_url
        if profile.tunnel.type == "cloudflare" and profile.tunnel.cloudflare_mode == "named":
            return profile.tunnel.public_url.rstrip("/")
        state = self._read_runtime_state(profile.id)
        value = state.get("public_url")
        return str(value).rstrip("/") if isinstance(value, str) and value.strip() else ""

    def resolved_endpoint(self, profile: WorkspaceProfile) -> str:
        public_url = self.resolved_public_url(profile)
        if not public_url:
            return ""
        return f"{public_url.rstrip('/')}/mcp"

    def _start_runtime_process(self, profile: WorkspaceProfile) -> tuple[subprocess.Popen[str], int]:
        command = self._resolve_command(profile)
        env = os.environ.copy()
        server_url = self._server_url_for_profile(profile)
        if server_url:
            env["CODING_TOOLS_MCP_SERVER_URL"] = server_url
        self._configure_pythonpath_for_local_repo(command, env)
        args = command + self._runtime_args(profile, env)

        log_dir = log_dir_for_profile(profile.id)
        stdout_path = log_dir / "stdout.log"
        stderr_path = log_dir / "stderr.log"
        stdout_handle = stdout_path.open("w", encoding="utf-8")
        stderr_handle = stderr_path.open("w", encoding="utf-8")
        popen_kwargs: dict[str, object] = {
            "args": args,
            "cwd": profile.path,
            "env": env,
            "stdout": stdout_handle,
            "stderr": stderr_handle,
            "text": True,
        }
        if os.name == "nt":
            popen_kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
        else:
            popen_kwargs["start_new_session"] = True
        process = subprocess.Popen(**popen_kwargs)
        if not self._wait_for_port_state(
            profile.runtime.local_port,
            listening=True,
            timeout=self.RUNTIME_START_TIMEOUT_SECONDS,
        ):
            self._terminate_process_tree(process.pid)
            raise RuntimeError(f"MCP 运行时没有在预期时间内监听端口 {profile.runtime.local_port}。")
        runtime_pid = self._find_pid_by_port(profile.runtime.local_port)
        if runtime_pid is None:
            self._terminate_process_tree(process.pid)
            raise RuntimeError(f"MCP 运行时已经启动，但未识别到端口 {profile.runtime.local_port} 对应的进程。")
        return process, runtime_pid

    def _start_cloudflare_tunnel(self, profile: WorkspaceProfile) -> tuple[subprocess.Popen[str], str]:
        cloudflared = self._find_cloudflared_command()
        if not cloudflared:
            raise RuntimeError("未找到 cloudflared。请先安装 Cloudflare Tunnel CLI，再使用 Cloudflare 方式。")

        log_dir = log_dir_for_profile(profile.id)
        tunnel_log = log_dir / "cloudflared.log"
        if profile.tunnel.cloudflare_mode == "named":
            if not profile.tunnel.cloudflare_token.strip():
                raise RuntimeError("Cloudflare 命名隧道模式需要填写 Tunnel Token。")
            if not profile.tunnel.public_url.strip():
                raise RuntimeError("Cloudflare 命名隧道模式需要填写固定公网地址。")
            args = [cloudflared, "tunnel", "run", "--token", profile.tunnel.cloudflare_token.strip()]
        else:
            args = [cloudflared, "tunnel", "--url", f"http://127.0.0.1:{profile.runtime.local_port}"]
        popen_kwargs: dict[str, object] = {
            "args": args,
            "cwd": profile.path,
            "stdout": subprocess.PIPE,
            "stderr": subprocess.STDOUT,
            "text": True,
            "encoding": "utf-8",
            "errors": "replace",
        }
        if os.name == "nt":
            popen_kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
        else:
            popen_kwargs["start_new_session"] = True
        process = subprocess.Popen(**popen_kwargs)
        public_url_holder: dict[str, str] = {"value": ""}
        ready = threading.Event()
        thread = threading.Thread(
            target=self._stream_cloudflare_output,
            args=(profile, process, tunnel_log, public_url_holder, ready),
            daemon=True,
        )
        thread.start()
        ready.wait(timeout=12)
        if profile.tunnel.cloudflare_mode == "named":
            return process, profile.tunnel.public_url.rstrip("/")
        if not public_url_holder["value"]:
            raise RuntimeError("cloudflared 已启动，但在预期时间内没有返回 trycloudflare.com 公网地址。")
        return process, public_url_holder["value"]

    def _stream_cloudflare_output(
        self,
        profile: WorkspaceProfile,
        process: subprocess.Popen[str],
        log_path: Path,
        public_url_holder: dict[str, str],
        ready: threading.Event,
    ) -> None:
        stream = process.stdout
        if stream is None:
            ready.set()
            return

        with log_path.open("a", encoding="utf-8") as handle:
            for raw_line in stream:
                line = raw_line.rstrip("\n")
                handle.write(raw_line)
                handle.flush()
                if profile.tunnel.cloudflare_mode == "named":
                    lowered = line.lower()
                    if "registered tunnel connection" in lowered or "starting metrics server" in lowered:
                        ready.set()
                    continue
                if not public_url_holder["value"]:
                    matched = TRYCLOUDFLARE_URL_RE.search(line)
                    if matched:
                        public_url_holder["value"] = matched.group(0).rstrip("/")
                        self._update_runtime_state(profile.id, public_url=public_url_holder["value"])
                        ready.set()
            ready.set()

    def _server_url_for_profile(self, profile: WorkspaceProfile) -> str:
        if profile.tunnel.type == "frp":
            return profile.effective_public_url
        if profile.tunnel.type == "cloudflare" and profile.tunnel.cloudflare_mode == "named":
            return profile.tunnel.public_url.rstrip("/")
        return ""

    def _validate_tunnel_requirements(self, profile: WorkspaceProfile) -> None:
        if profile.tunnel.type == "frp":
            return
        if profile.tunnel.type != "cloudflare":
            raise RuntimeError("当前仅支持 FRP 和 Cloudflare。")
        if not self._find_cloudflared_command():
            raise RuntimeError(
                "未找到 cloudflared。请先安装 Cloudflare Tunnel CLI。\n"
                "Windows 可执行：winget install Cloudflare.cloudflared"
            )
        if profile.tunnel.cloudflare_mode == "named":
            if not profile.tunnel.cloudflare_token.strip():
                raise RuntimeError("Cloudflare 固定域名模式需要填写 Tunnel Token。")
            if not profile.tunnel.public_url.strip():
                raise RuntimeError("Cloudflare 固定域名模式需要填写公网地址。")

    def _append_tunnel_error_log(self, profile: WorkspaceProfile, message: str) -> None:
        path = log_dir_for_profile(profile.id) / "cloudflared.log"
        timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
        with path.open("a", encoding="utf-8") as handle:
            handle.write(f"[{timestamp}] {message}\n")

    def _resolve_command(self, profile: WorkspaceProfile) -> list[str]:
        if profile.runtime.runtime_command.strip():
            return profile.runtime.runtime_command.strip().split()
        if shutil.which("coding-tools-mcp"):
            return ["coding-tools-mcp"]
        repo_root = Path(__file__).resolve().parents[3]
        if (repo_root / "coding_tools_mcp").exists():
            return [sys.executable, "-m", "coding_tools_mcp"]
        if shutil.which("uvx"):
            return ["uvx", "coding-tools-mcp"]
        raise RuntimeError("未找到 uvx、coding-tools-mcp，或本地模块入口。")

    def _runtime_args(self, profile: WorkspaceProfile, env: dict[str, str]) -> list[str]:
        args = [
            "--workspace",
            profile.path,
            "--host",
            "127.0.0.1",
            "--port",
            str(profile.runtime.local_port),
            "--tool-profile",
            profile.runtime.tool_profile,
            "--permission-mode",
            profile.runtime.permission_mode,
        ]
        if profile.auth.type == "oauth":
            env["CODING_TOOLS_MCP_OAUTH_CLIENT_ID"] = profile.auth.oauth_client_id
            if profile.auth.oauth_client_secret.strip():
                env["CODING_TOOLS_MCP_OAUTH_CLIENT_SECRET"] = profile.auth.oauth_client_secret
            else:
                env.pop("CODING_TOOLS_MCP_OAUTH_CLIENT_SECRET", None)
            env["CODING_TOOLS_MCP_OAUTH_PASSWORD"] = profile.auth.oauth_password
            env["CODING_TOOLS_MCP_OAUTH_TOKEN_SECRET"] = profile.auth.oauth_token_secret
            args.append("--oauth-mode")
        elif profile.auth.type == "bearer":
            args.extend(["--auth-token", profile.auth.bearer_token])
        elif profile.auth.type == "noauth":
            env["CODING_TOOLS_MCP_AUTH_MODE"] = "noauth"
        return args

    def _configure_pythonpath_for_local_repo(self, command: list[str], env: dict[str, str]) -> None:
        if command[:2] != [sys.executable, "-m"]:
            return
        repo_root = str(Path(__file__).resolve().parents[3])
        current = env.get("PYTHONPATH", "").strip()
        env["PYTHONPATH"] = repo_root if not current else os.pathsep.join([repo_root, current])

    def _state_file(self, profile_id: str) -> Path:
        return runtime_state_file_for_profile(profile_id)

    def _write_runtime_state(
        self,
        profile: WorkspaceProfile,
        *,
        runtime_pid: int | None,
        tunnel_pid: int | None = None,
        public_url: str = "",
    ) -> None:
        payload = {
            "runtime_pid": runtime_pid,
            "tunnel_pid": tunnel_pid,
            "port": profile.runtime.local_port,
            "workspace": profile.path,
            "tunnel_type": profile.tunnel.type,
            "public_url": public_url.rstrip("/"),
        }
        self._state_file(profile.id).write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    def _update_runtime_state(self, profile_id: str, **updates: object) -> None:
        state = self._read_runtime_state(profile_id)
        if not state:
            return
        state.update(updates)
        self._state_file(profile_id).write_text(json.dumps(state, indent=2) + "\n", encoding="utf-8")

    def _clear_runtime_state(self, profile_id: str) -> None:
        path = self._state_file(profile_id)
        if path.exists():
            path.unlink()

    def _read_runtime_state(self, profile_id: str) -> dict[str, object]:
        path = self._state_file(profile_id)
        if not path.exists():
            return {}
        try:
            return json.loads(path.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            return {}

    def _find_runtime_pid(self, profile: WorkspaceProfile) -> int | None:
        session = self._sessions.get(profile.id)
        if session and session.runtime_pid is not None and self._process_matches_profile(session.runtime_pid, profile):
            return session.runtime_pid
        if session and session.runtime_process and session.runtime_process.poll() is None:
            process_pid = session.runtime_process.pid
            if self._process_matches_profile(process_pid, profile):
                return process_pid
        port_pid = self._find_pid_by_port(profile.runtime.local_port)
        if port_pid is not None and self._process_matches_profile(port_pid, profile):
            return port_pid
        state = self._read_runtime_state(profile.id)
        runtime_pid = state.get("runtime_pid", state.get("pid"))
        if isinstance(runtime_pid, int) and self._process_matches_profile(runtime_pid, profile):
            return runtime_pid
        return None

    def _find_tunnel_pid(self, profile: WorkspaceProfile) -> int | None:
        session = self._sessions.get(profile.id)
        if session and session.tunnel_process and session.tunnel_process.poll() is None:
            return session.tunnel_process.pid
        state = self._read_runtime_state(profile.id)
        tunnel_pid = state.get("tunnel_pid")
        if isinstance(tunnel_pid, int) and self._process_is_alive(tunnel_pid):
            return tunnel_pid
        if profile.tunnel.type == "cloudflare" and profile.tunnel.cloudflare_mode == "quick":
            return self._find_cloudflare_quick_tunnel_pid(profile.runtime.local_port)
        return None

    def _process_matches_profile(self, pid: int, profile: WorkspaceProfile) -> bool:
        command_line = self._command_line_for_pid(pid)
        if not command_line:
            return False
        normalized_command = command_line.replace("\\", "/").lower()
        normalized_workspace = profile.path.replace("\\", "/").lower()
        return (
            "coding-tools-mcp" in normalized_command
            and f"--port {profile.runtime.local_port}" in normalized_command
            and normalized_workspace in normalized_command
        )

    def _process_is_alive(self, pid: int) -> bool:
        return self._safe_process(pid) is not None

    def _find_pid_by_port(self, port: int) -> int | None:
        try:
            for connection in psutil.net_connections(kind="tcp"):
                if connection.status != psutil.CONN_LISTEN:
                    continue
                if not connection.laddr or connection.laddr.port != port:
                    continue
                if connection.pid:
                    return connection.pid
        except (psutil.AccessDenied, psutil.Error):
            return None
        return None

    def _wait_for_port_state(self, port: int, listening: bool, timeout: float) -> bool:
        deadline = time.time() + timeout
        while time.time() < deadline:
            is_listening = self._find_pid_by_port(port) is not None
            if is_listening == listening:
                return True
            time.sleep(0.2)
        return False

    def _command_line_for_pid(self, pid: int) -> str:
        process = self._safe_process(pid)
        if process is None:
            return ""
        try:
            return " ".join(process.cmdline()).strip()
        except (psutil.AccessDenied, psutil.Error):
            return ""

    def _terminate_process_tree(self, pid: int) -> None:
        try:
            process = psutil.Process(pid)
        except psutil.Error:
            return

        try:
            children = process.children(recursive=True)
        except (psutil.AccessDenied, psutil.Error):
            children = []

        for child in reversed(children):
            try:
                child.terminate()
            except psutil.Error:
                continue
        try:
            process.terminate()
        except psutil.Error:
            pass

        _gone, alive = psutil.wait_procs(children + [process], timeout=3)
        if alive:
            for item in alive:
                try:
                    item.kill()
                except psutil.Error:
                    continue
            psutil.wait_procs(alive, timeout=2)

    def _safe_process(self, pid: int) -> psutil.Process | None:
        try:
            return psutil.Process(pid)
        except psutil.Error:
            return None

    def _cleanup_orphan_tunnel(self, profile: WorkspaceProfile) -> None:
        if self._find_runtime_pid(profile) is not None:
            return
        tunnel_pid = self._find_tunnel_pid(profile)
        if tunnel_pid is None:
            return
        self._terminate_process_tree(tunnel_pid)
        self._clear_runtime_state(profile.id)

    def _find_cloudflare_quick_tunnel_pid(self, port: int) -> int | None:
        target = f"http://127.0.0.1:{port}".lower()
        for process in psutil.process_iter(["pid", "name", "cmdline"]):
            try:
                command_line = " ".join(process.info.get("cmdline") or []).lower()
            except (psutil.AccessDenied, psutil.Error):
                continue
            if "cloudflared" not in command_line:
                continue
            if target in command_line:
                return process.info["pid"]
        return None

    def _find_cloudflared_command(self) -> str | None:
        direct = shutil.which("cloudflared")
        if direct:
            return direct
        candidates = [
            Path(r"C:\Program Files\cloudflared\cloudflared.exe"),
            Path(r"C:\Program Files (x86)\cloudflared\cloudflared.exe"),
            Path.home() / ".cloudflared" / "cloudflared.exe",
        ]
        for candidate in candidates:
            if candidate.exists():
                return str(candidate)
        return None
