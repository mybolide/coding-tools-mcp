from __future__ import annotations

import sys
from pathlib import Path

from PySide6.QtCore import QObject, QSignalBlocker, QThread, QTimer, Qt, Signal
from PySide6.QtWidgets import (
    QApplication,
    QFileDialog,
    QComboBox,
    QFormLayout,
    QFrame,
    QGridLayout,
    QGroupBox,
    QHBoxLayout,
    QLabel,
    QLineEdit,
    QListWidget,
    QListWidgetItem,
    QMainWindow,
    QMessageBox,
    QPushButton,
    QSpinBox,
    QTabWidget,
    QTextEdit,
    QVBoxLayout,
    QWidget,
)

from .actions_runtime import ActionsRuntimeManager
from .models import RuntimeStatus, WorkspaceProfile, build_profile
from .runtime import RuntimeManager
from .storage import load_profiles, log_dir_for_profile, save_profiles
from .theme import STYLESHEET


class RuntimeJob(QObject):
    finished = Signal(str, object, str)

    def __init__(self, runtime: object, profile: WorkspaceProfile, action: str) -> None:
        super().__init__()
        self.runtime = runtime
        self.profile = profile
        self.action = action

    def run(self) -> None:
        try:
            if self.action == "start":
                status = self.runtime.start(self.profile)  # type: ignore[call-arg]
            else:
                status = self.runtime.stop(self.profile)  # type: ignore[call-arg]
            self.finished.emit(self.action, status, "")
        except Exception as exc:  # noqa: BLE001
            self.finished.emit(self.action, None, str(exc))


class MainWindow(QMainWindow):
    TUNNEL_OPTIONS = [
        ("frp", "FRP"),
        ("cloudflare", "Cloudflare"),
    ]
    CLOUDFLARE_MODE_OPTIONS = [
        ("quick", "临时隧道"),
        ("named", "固定域名"),
    ]
    AUTH_OPTIONS = [
        ("oauth", "OAuth"),
        ("bearer", "Bearer Token"),
        ("noauth", "不启用认证"),
    ]
    ACTIONS_AUTH_OPTIONS = [
        ("api_key", "API Key / Bearer"),
        ("none", "不启用认证"),
        ("oauth", "OAuth（参数预留）"),
    ]
    TOKEN_EXCHANGE_OPTIONS = [
        ("authorization_header", "请求头"),
        ("request_body", "请求体"),
    ]
    TOOL_PROFILE_OPTIONS = [
        ("full", "完整工具"),
        ("read-only", "只读工具"),
        ("compat-readonly-all", "兼容只读"),
    ]
    PERMISSION_MODE_OPTIONS = [
        ("trusted", "受信任"),
        ("safe", "安全受限"),
        ("dangerous", "完全放开"),
    ]

    def __init__(self) -> None:
        super().__init__()
        self.setWindowTitle("Coding Tools MCP Desktop")
        self.resize(1540, 980)
        self.mcp_runtime = RuntimeManager()
        self.actions_runtime = ActionsRuntimeManager()
        self.profiles = load_profiles()
        self.current_profile: WorkspaceProfile | None = None
        self._runtime_thread: QThread | None = None
        self._runtime_job: RuntimeJob | None = None
        self._busy_profile_id: str | None = None
        self._busy_action: str | None = None
        self._busy_service: str | None = None
        self._busy_dots = 0
        self._loading_profile = False
        self._busy_timer = QTimer(self)
        self._busy_timer.setInterval(350)
        self._busy_timer.timeout.connect(self._tick_busy_indicator)
        self._build_ui()
        self._populate_workspace_list()
        if self.profiles:
            self.workspace_list.setCurrentRow(0)
        else:
            self._clear_panel()

    def _build_ui(self) -> None:
        root = QWidget()
        layout = QHBoxLayout(root)
        layout.setContentsMargins(18, 18, 18, 18)
        layout.setSpacing(18)

        sidebar = QFrame()
        sidebar.setObjectName("Sidebar")
        sidebar_layout = QVBoxLayout(sidebar)
        sidebar_layout.setContentsMargins(18, 18, 18, 18)
        sidebar_layout.setSpacing(14)

        eyebrow = QLabel("工作区控制台")
        eyebrow.setObjectName("Eyebrow")
        title = QLabel("MCP / Actions 桌面客户端")
        title.setObjectName("Title")
        subtitle = QLabel("一个 Workspace，同时管理 MCP 和 GPT Actions 两套入口。")
        subtitle.setWordWrap(True)
        subtitle.setStyleSheet("color:#667085; font-size:14px;")

        actions = QHBoxLayout()
        add_button = QPushButton("添加工作区")
        add_button.clicked.connect(self._add_workspace)
        self.delete_button = QPushButton("删除")
        self.delete_button.setProperty("secondary", True)
        self.delete_button.clicked.connect(self._delete_workspace)
        refresh_button = QPushButton("刷新")
        refresh_button.setProperty("secondary", True)
        refresh_button.clicked.connect(self._refresh_current)
        actions.addWidget(add_button)
        actions.addWidget(self.delete_button)
        actions.addWidget(refresh_button)

        self.workspace_list = QListWidget()
        self.workspace_list.currentRowChanged.connect(self._on_workspace_selected)

        sidebar_layout.addWidget(eyebrow)
        sidebar_layout.addWidget(title)
        sidebar_layout.addWidget(subtitle)
        sidebar_layout.addLayout(actions)
        sidebar_layout.addWidget(self.workspace_list, 1)

        panel = QFrame()
        panel.setObjectName("Panel")
        panel_layout = QVBoxLayout(panel)
        panel_layout.setContentsMargins(22, 22, 22, 22)
        panel_layout.setSpacing(16)

        self.header_title = QLabel("先添加一个工作区")
        self.header_title.setObjectName("Title")
        self.header_title.setStyleSheet("font-size:24px;")
        self.header_meta = QLabel("左侧添加工作区后，再配置 MCP 或 Actions。")
        self.header_meta.setStyleSheet("color:#667085; font-size:13px;")

        header_actions = QHBoxLayout()
        self.start_button = QPushButton("启动")
        self.start_button.clicked.connect(self._start_runtime)
        self.stop_button = QPushButton("停止")
        self.stop_button.setProperty("secondary", True)
        self.stop_button.clicked.connect(self._stop_runtime)
        self.copy_button = QPushButton("复制 MCP 地址")
        self.copy_button.setProperty("secondary", True)
        self.copy_button.clicked.connect(self._copy_endpoint)
        self.copy_frp_button = QPushButton("复制 FRP 片段")
        self.copy_frp_button.setProperty("secondary", True)
        self.copy_frp_button.clicked.connect(self._copy_frp_snippet)
        header_actions.addWidget(self.start_button)
        header_actions.addWidget(self.stop_button)
        header_actions.addWidget(self.copy_button)
        header_actions.addWidget(self.copy_frp_button)
        header_actions.addStretch(1)

        self.service_tabs = QTabWidget()
        self.service_tabs.currentChanged.connect(self._on_service_tab_changed)
        self.service_tabs.addTab(self._build_mcp_tab(), "MCP")
        self.service_tabs.addTab(self._build_actions_tab(), "Actions")

        panel_layout.addWidget(self.header_title)
        panel_layout.addWidget(self.header_meta)
        panel_layout.addLayout(header_actions)
        panel_layout.addWidget(self.service_tabs, 1)

        layout.addWidget(sidebar, 1)
        layout.addWidget(panel, 2)
        self.setCentralWidget(root)
        self._wire_live_updates()

    def _build_mcp_tab(self) -> QWidget:
        page = QWidget()
        content = QGridLayout(page)
        content.setHorizontalSpacing(16)
        content.setVerticalSpacing(16)
        self.mcp_workspace_group = self._build_mcp_workspace_group()
        self.mcp_runtime_group = self._build_mcp_runtime_group()
        self.mcp_auth_group = self._build_mcp_auth_group()
        self.mcp_log_group = self._build_mcp_log_group()
        content.addWidget(self.mcp_workspace_group, 0, 0)
        content.addWidget(self.mcp_runtime_group, 0, 1)
        content.addWidget(self.mcp_auth_group, 1, 0)
        content.addWidget(self.mcp_log_group, 1, 1)
        content.setColumnStretch(0, 1)
        content.setColumnStretch(1, 1)
        return page

    def _build_actions_tab(self) -> QWidget:
        page = QWidget()
        content = QGridLayout(page)
        content.setHorizontalSpacing(16)
        content.setVerticalSpacing(16)
        self.actions_workspace_group = self._build_actions_workspace_group()
        self.actions_runtime_group = self._build_actions_runtime_group()
        self.actions_auth_group = self._build_actions_auth_group()
        self.actions_log_group = self._build_actions_log_group()
        content.addWidget(self.actions_workspace_group, 0, 0)
        content.addWidget(self.actions_runtime_group, 0, 1)
        content.addWidget(self.actions_auth_group, 1, 0)
        content.addWidget(self.actions_log_group, 1, 1)
        content.setColumnStretch(0, 1)
        content.setColumnStretch(1, 1)
        return page

    def _build_mcp_workspace_group(self) -> QGroupBox:
        box = QGroupBox("MCP 工作区与公网入口")
        self.mcp_workspace_form = QFormLayout(box)
        self.name_edit = QLineEdit()
        self.path_edit = QLineEdit()
        self.path_edit.setReadOnly(True)
        self.tunnel_type = QComboBox()
        self._fill_combo(self.tunnel_type, self.TUNNEL_OPTIONS)
        self.tunnel_type.currentIndexChanged.connect(self._refresh_mcp_tunnel_fields)
        self.public_url_label = QLabel("公网地址")
        self.public_url_edit = QLineEdit()
        self.public_url_edit.setPlaceholderText("Cloudflare 启动后会自动分配公网地址")
        self.cloudflare_mode_label = QLabel("Cloudflare 模式")
        self.cloudflare_mode = QComboBox()
        self._fill_combo(self.cloudflare_mode, self.CLOUDFLARE_MODE_OPTIONS)
        self.cloudflare_mode.currentIndexChanged.connect(self._refresh_mcp_tunnel_fields)
        self.cloudflare_token_label = QLabel("Tunnel Token")
        self.cloudflare_token_edit = QLineEdit()
        self.frp_server_label = QLabel("FRP 服务器域名")
        self.frp_server_edit = QLineEdit()
        self.frp_server_edit.setPlaceholderText("例如：frp.example.com")
        self.subdomain_label = QLabel("FRP 子域名")
        self.subdomain_edit = QLineEdit()
        self.subdomain_edit.setPlaceholderText("例如：mcp")
        self.endpoint_hint = QLabel("当前入口：-")
        self.endpoint_hint.setWordWrap(True)
        self.endpoint_hint.setStyleSheet("color:#667085;")
        save_button = QPushButton("保存当前工作区配置")
        save_button.clicked.connect(self._save_current)

        self.mcp_workspace_form.addRow("名称", self.name_edit)
        self.mcp_workspace_form.addRow("工作区路径", self.path_edit)
        self.mcp_workspace_form.addRow("隧道方式", self.tunnel_type)
        self.mcp_workspace_form.addRow(self.cloudflare_mode_label, self.cloudflare_mode)
        self.mcp_workspace_form.addRow(self.public_url_label, self.public_url_edit)
        self.mcp_workspace_form.addRow(self.cloudflare_token_label, self.cloudflare_token_edit)
        self.mcp_workspace_form.addRow(self.frp_server_label, self.frp_server_edit)
        self.mcp_workspace_form.addRow(self.subdomain_label, self.subdomain_edit)
        self.mcp_workspace_form.addRow("当前入口", self.endpoint_hint)
        self.mcp_workspace_form.addRow(save_button)
        return box

    def _build_mcp_runtime_group(self) -> QGroupBox:
        box = QGroupBox("MCP 运行时")
        form = QFormLayout(box)
        self.local_port = QSpinBox()
        self.local_port.setRange(1000, 65535)
        self.tool_profile = QComboBox()
        self._fill_combo(self.tool_profile, self.TOOL_PROFILE_OPTIONS)
        self.permission_mode = QComboBox()
        self._fill_combo(self.permission_mode, self.PERMISSION_MODE_OPTIONS)
        self.runtime_command = QLineEdit()
        self.runtime_command.setPlaceholderText("可选，例如：coding-tools-mcp")
        self.status_label = QLabel("未启动")
        self.status_label.setStyleSheet("font-weight:700; color:#b42318;")
        form.addRow("本地端口", self.local_port)
        form.addRow("工具档位", self.tool_profile)
        form.addRow("权限模式", self.permission_mode)
        form.addRow("自定义命令", self.runtime_command)
        form.addRow("状态", self.status_label)
        return box

    def _build_mcp_auth_group(self) -> QGroupBox:
        box = QGroupBox("MCP 认证与 ChatGPT 接入")
        layout = QVBoxLayout(box)
        self.auth_form = QFormLayout()
        self.auth_type = QComboBox()
        self._fill_combo(self.auth_type, self.AUTH_OPTIONS)
        self.auth_type.currentIndexChanged.connect(self._refresh_mcp_auth_fields)
        self.oauth_client_id_label = QLabel("OAuth 客户端 ID")
        self.oauth_client_id = QLineEdit()
        self.oauth_client_secret_label = QLabel("OAuth 客户端密钥")
        self.oauth_client_secret = QLineEdit()
        self.oauth_password_label = QLabel("授权口令")
        self.oauth_password = QLineEdit()
        self.oauth_password.setPlaceholderText("ChatGPT 首次授权时输入这个口令")
        self.bearer_token_label = QLabel("Bearer Token")
        self.bearer_token = QLineEdit()
        self.auth_form.addRow("认证方式", self.auth_type)
        self.auth_form.addRow(self.oauth_client_id_label, self.oauth_client_id)
        self.auth_form.addRow(self.oauth_client_secret_label, self.oauth_client_secret)
        self.auth_form.addRow(self.oauth_password_label, self.oauth_password)
        self.auth_form.addRow(self.bearer_token_label, self.bearer_token)
        layout.addLayout(self.auth_form)

        self.oauth_actions = QWidget()
        oauth_actions_layout = QHBoxLayout(self.oauth_actions)
        oauth_actions_layout.setContentsMargins(0, 0, 0, 0)
        oauth_actions_layout.setSpacing(10)
        self.copy_client_id_button = QPushButton("复制客户端 ID")
        self.copy_client_id_button.setProperty("secondary", True)
        self.copy_client_id_button.clicked.connect(self._copy_oauth_client_id)
        self.copy_client_secret_button = QPushButton("复制客户端密钥")
        self.copy_client_secret_button.setProperty("secondary", True)
        self.copy_client_secret_button.clicked.connect(self._copy_oauth_client_secret)
        self.copy_oauth_password_button = QPushButton("复制授权口令")
        self.copy_oauth_password_button.setProperty("secondary", True)
        self.copy_oauth_password_button.clicked.connect(self._copy_oauth_password)
        oauth_actions_layout.addWidget(self.copy_client_id_button)
        oauth_actions_layout.addWidget(self.copy_client_secret_button)
        oauth_actions_layout.addWidget(self.copy_oauth_password_button)
        oauth_actions_layout.addStretch(1)
        layout.addWidget(self.oauth_actions)

        self.bearer_actions = QWidget()
        bearer_actions_layout = QHBoxLayout(self.bearer_actions)
        bearer_actions_layout.setContentsMargins(0, 0, 0, 0)
        bearer_actions_layout.setSpacing(10)
        self.copy_bearer_button = QPushButton("复制 Bearer Token")
        self.copy_bearer_button.setProperty("secondary", True)
        self.copy_bearer_button.clicked.connect(self._copy_bearer_token)
        bearer_actions_layout.addWidget(self.copy_bearer_button)
        bearer_actions_layout.addStretch(1)
        layout.addWidget(self.bearer_actions)

        self.auth_hint = QLabel("OAuth 模式下，ChatGPT 里填客户端 ID 和客户端密钥；首次授权时再输入授权口令。")
        self.auth_hint.setWordWrap(True)
        self.auth_hint.setStyleSheet("color:#667085;")
        layout.addWidget(self.auth_hint)
        return box

    def _build_mcp_log_group(self) -> QGroupBox:
        box = QGroupBox("MCP 日志与地址")
        layout = QVBoxLayout(box)
        self.endpoint_label = QLabel("公网 MCP 地址：-")
        self.local_label = QLabel("本地 MCP 地址：-")
        self.log_output = QTextEdit()
        self.log_output.setReadOnly(True)
        self.log_output.setMinimumHeight(220)
        layout.addWidget(self.endpoint_label)
        layout.addWidget(self.local_label)
        layout.addWidget(self.log_output)
        return box

    def _build_actions_workspace_group(self) -> QGroupBox:
        box = QGroupBox("Actions 工作区与公网入口")
        self.actions_workspace_form = QFormLayout(box)
        self.actions_tunnel_type = QComboBox()
        self._fill_combo(self.actions_tunnel_type, self.TUNNEL_OPTIONS)
        self.actions_tunnel_type.currentIndexChanged.connect(self._refresh_actions_tunnel_fields)
        self.actions_public_url_label = QLabel("公网地址")
        self.actions_public_url_edit = QLineEdit()
        self.actions_public_url_edit.setPlaceholderText("例如：https://actions.example.com")
        self.actions_cloudflare_mode_label = QLabel("Cloudflare 模式")
        self.actions_cloudflare_mode = QComboBox()
        self._fill_combo(self.actions_cloudflare_mode, self.CLOUDFLARE_MODE_OPTIONS)
        self.actions_cloudflare_mode.currentIndexChanged.connect(self._refresh_actions_tunnel_fields)
        self.actions_cloudflare_token_label = QLabel("Tunnel Token")
        self.actions_cloudflare_token_edit = QLineEdit()
        self.actions_frp_server_label = QLabel("FRP 服务器域名")
        self.actions_frp_server_edit = QLineEdit()
        self.actions_frp_server_edit.setPlaceholderText("例如：frp.example.com")
        self.actions_subdomain_label = QLabel("FRP 子域名")
        self.actions_subdomain_edit = QLineEdit()
        self.actions_subdomain_edit.setPlaceholderText("例如：actions")
        self.actions_endpoint_hint = QLabel("OpenAPI：-")
        self.actions_endpoint_hint.setWordWrap(True)
        self.actions_endpoint_hint.setStyleSheet("color:#667085;")
        self.actions_workspace_form.addRow("隧道方式", self.actions_tunnel_type)
        self.actions_workspace_form.addRow(self.actions_cloudflare_mode_label, self.actions_cloudflare_mode)
        self.actions_workspace_form.addRow(self.actions_public_url_label, self.actions_public_url_edit)
        self.actions_workspace_form.addRow(self.actions_cloudflare_token_label, self.actions_cloudflare_token_edit)
        self.actions_workspace_form.addRow(self.actions_frp_server_label, self.actions_frp_server_edit)
        self.actions_workspace_form.addRow(self.actions_subdomain_label, self.actions_subdomain_edit)
        self.actions_workspace_form.addRow("OpenAPI 入口", self.actions_endpoint_hint)
        return box

    def _build_actions_runtime_group(self) -> QGroupBox:
        box = QGroupBox("Actions 运行时")
        form = QFormLayout(box)
        self.actions_local_port = QSpinBox()
        self.actions_local_port.setRange(1000, 65535)
        self.actions_permission_mode = QComboBox()
        self._fill_combo(self.actions_permission_mode, self.PERMISSION_MODE_OPTIONS)
        self.actions_runtime_command = QLineEdit()
        self.actions_runtime_command.setPlaceholderText("可选，例如：coding-tools-actions")
        self.actions_allowed_commands = QLineEdit()
        self.actions_allowed_commands.setPlaceholderText("逗号分隔，例如：pytest,python,ruff")
        self.actions_max_patch_bytes = QSpinBox()
        self.actions_max_patch_bytes.setRange(1024, 5_000_000)
        self.actions_status_label = QLabel("未启动")
        self.actions_status_label.setStyleSheet("font-weight:700; color:#b42318;")
        form.addRow("本地端口", self.actions_local_port)
        form.addRow("权限模式", self.actions_permission_mode)
        form.addRow("自定义命令", self.actions_runtime_command)
        form.addRow("允许命令", self.actions_allowed_commands)
        form.addRow("最大 Patch 字节数", self.actions_max_patch_bytes)
        form.addRow("状态", self.actions_status_label)
        return box

    def _build_actions_auth_group(self) -> QGroupBox:
        box = QGroupBox("Actions 鉴权与 GPT 接入")
        layout = QVBoxLayout(box)
        form = QFormLayout()
        self.actions_auth_type = QComboBox()
        self._fill_combo(self.actions_auth_type, self.ACTIONS_AUTH_OPTIONS)
        self.actions_auth_type.currentIndexChanged.connect(self._refresh_actions_auth_fields)
        self.actions_api_key_label = QLabel("API Key（Bearer）")
        self.actions_api_key = QLineEdit()
        self.actions_oauth_client_id_label = QLabel("OAuth 客户端 ID")
        self.actions_oauth_client_id = QLineEdit()
        self.actions_oauth_client_secret_label = QLabel("OAuth 客户端密钥")
        self.actions_oauth_client_secret = QLineEdit()
        self.actions_oauth_authorization_url_label = QLabel("授权 URL")
        self.actions_oauth_authorization_url = QLineEdit()
        self.actions_oauth_authorization_url.setPlaceholderText("例如：https://actions.example.com/oauth/authorize")
        self.actions_oauth_token_url_label = QLabel("令牌 URL")
        self.actions_oauth_token_url = QLineEdit()
        self.actions_oauth_token_url.setPlaceholderText("例如：https://actions.example.com/oauth/token")
        self.actions_oauth_scopes_label = QLabel("Scope")
        self.actions_oauth_scopes = QLineEdit()
        self.actions_oauth_scopes.setPlaceholderText("多个 scope 用空格分隔")
        self.actions_oauth_token_exchange_method_label = QLabel("Token 交换方式")
        self.actions_oauth_token_exchange_method = QComboBox()
        self._fill_combo(self.actions_oauth_token_exchange_method, self.TOKEN_EXCHANGE_OPTIONS)
        form.addRow("认证方式", self.actions_auth_type)
        form.addRow(self.actions_api_key_label, self.actions_api_key)
        form.addRow(self.actions_oauth_client_id_label, self.actions_oauth_client_id)
        form.addRow(self.actions_oauth_client_secret_label, self.actions_oauth_client_secret)
        form.addRow(self.actions_oauth_authorization_url_label, self.actions_oauth_authorization_url)
        form.addRow(self.actions_oauth_token_url_label, self.actions_oauth_token_url)
        form.addRow(self.actions_oauth_scopes_label, self.actions_oauth_scopes)
        form.addRow(
            self.actions_oauth_token_exchange_method_label,
            self.actions_oauth_token_exchange_method,
        )
        layout.addLayout(form)

        self.actions_api_key_actions = QWidget()
        actions_layout = QHBoxLayout(self.actions_api_key_actions)
        actions_layout.setContentsMargins(0, 0, 0, 0)
        actions_layout.setSpacing(10)
        self.copy_actions_api_key_button = QPushButton("复制 API Key")
        self.copy_actions_api_key_button.setProperty("secondary", True)
        self.copy_actions_api_key_button.clicked.connect(self._copy_actions_api_key)
        actions_layout.addWidget(self.copy_actions_api_key_button)
        actions_layout.addStretch(1)
        layout.addWidget(self.actions_api_key_actions)

        self.actions_oauth_actions = QWidget()
        oauth_actions_layout = QHBoxLayout(self.actions_oauth_actions)
        oauth_actions_layout.setContentsMargins(0, 0, 0, 0)
        oauth_actions_layout.setSpacing(10)
        self.copy_actions_oauth_client_id_button = QPushButton("复制 Client ID")
        self.copy_actions_oauth_client_id_button.setProperty("secondary", True)
        self.copy_actions_oauth_client_id_button.clicked.connect(self._copy_actions_oauth_client_id)
        self.copy_actions_oauth_client_secret_button = QPushButton("复制 Client Secret")
        self.copy_actions_oauth_client_secret_button.setProperty("secondary", True)
        self.copy_actions_oauth_client_secret_button.clicked.connect(self._copy_actions_oauth_client_secret)
        self.copy_actions_oauth_authorization_url_button = QPushButton("复制授权 URL")
        self.copy_actions_oauth_authorization_url_button.setProperty("secondary", True)
        self.copy_actions_oauth_authorization_url_button.clicked.connect(self._copy_actions_oauth_authorization_url)
        self.copy_actions_oauth_token_url_button = QPushButton("复制令牌 URL")
        self.copy_actions_oauth_token_url_button.setProperty("secondary", True)
        self.copy_actions_oauth_token_url_button.clicked.connect(self._copy_actions_oauth_token_url)
        oauth_actions_layout.addWidget(self.copy_actions_oauth_client_id_button)
        oauth_actions_layout.addWidget(self.copy_actions_oauth_client_secret_button)
        oauth_actions_layout.addWidget(self.copy_actions_oauth_authorization_url_button)
        oauth_actions_layout.addWidget(self.copy_actions_oauth_token_url_button)
        oauth_actions_layout.addStretch(1)
        layout.addWidget(self.actions_oauth_actions)

        actions = QWidget()
        common_actions_layout = QHBoxLayout(actions)
        common_actions_layout.setContentsMargins(0, 0, 0, 0)
        common_actions_layout.setSpacing(10)
        self.copy_actions_openapi_button = QPushButton("复制 OpenAPI 地址")
        self.copy_actions_openapi_button.setProperty("secondary", True)
        self.copy_actions_openapi_button.clicked.connect(self._copy_endpoint)
        self.copy_actions_privacy_button = QPushButton("复制隐私政策地址")
        self.copy_actions_privacy_button.setProperty("secondary", True)
        self.copy_actions_privacy_button.clicked.connect(self._copy_actions_privacy_url)
        common_actions_layout.addWidget(self.copy_actions_openapi_button)
        common_actions_layout.addWidget(self.copy_actions_privacy_button)
        common_actions_layout.addStretch(1)
        layout.addWidget(actions)

        self.actions_auth_hint = QLabel(
            "在私有 GPT 里选择 API Key，认证类型选 Bearer，Key 直接填这里的 API Key。"
        )
        self.actions_auth_hint.setWordWrap(True)
        self.actions_auth_hint.setStyleSheet("color:#667085;")
        layout.addWidget(self.actions_auth_hint)
        return box

    def _build_actions_log_group(self) -> QGroupBox:
        box = QGroupBox("Actions 日志与地址")
        layout = QVBoxLayout(box)
        self.actions_openapi_label = QLabel("OpenAPI 地址：-")
        self.actions_privacy_label = QLabel("隐私政策地址：-")
        self.actions_local_label = QLabel("本地 Actions 地址：-")
        self.actions_log_output = QTextEdit()
        self.actions_log_output.setReadOnly(True)
        self.actions_log_output.setMinimumHeight(220)
        layout.addWidget(self.actions_openapi_label)
        layout.addWidget(self.actions_privacy_label)
        layout.addWidget(self.actions_local_label)
        layout.addWidget(self.actions_log_output)
        return box

    def _wire_live_updates(self) -> None:
        for widget in (
            self.public_url_edit,
            self.frp_server_edit,
            self.subdomain_edit,
            self.actions_public_url_edit,
            self.actions_frp_server_edit,
            self.actions_subdomain_edit,
        ):
            widget.textChanged.connect(self._refresh_connection_view)
        for widget in (self.local_port, self.actions_local_port):
            widget.valueChanged.connect(self._refresh_connection_view)

    def _populate_workspace_list(self) -> None:
        self.workspace_list.clear()
        for profile in self.profiles:
            item = QListWidgetItem(self._workspace_summary(profile))
            item.setData(Qt.ItemDataRole.UserRole, profile.id)
            self.workspace_list.addItem(item)

    def _on_workspace_selected(self, row: int) -> None:
        if row < 0 or row >= len(self.profiles):
            self.current_profile = None
            self._clear_panel()
            return
        self.current_profile = self.profiles[row]
        self._load_profile(self.current_profile)

    def _on_service_tab_changed(self, _index: int) -> None:
        self._update_header_for_active_tab()
        if self.current_profile is not None:
            self._refresh_connection_view()

    def _load_profile(self, profile: WorkspaceProfile) -> None:
        self._loading_profile = True
        blockers = [QSignalBlocker(widget) for widget in self._form_widgets()]
        try:
            self.header_title.setText(profile.name)
            self.header_meta.setText(profile.path)

            self.name_edit.setText(profile.name)
            self.path_edit.setText(profile.path)
            self._set_combo_value(self.tunnel_type, profile.tunnel.type)
            self._set_combo_value(self.cloudflare_mode, profile.tunnel.cloudflare_mode)
            self.public_url_edit.setText(self._profile_public_url_for_edit(profile))
            self.cloudflare_token_edit.setText(profile.tunnel.cloudflare_token)
            self.frp_server_edit.setText(profile.tunnel.frp_server)
            self.subdomain_edit.setText(profile.tunnel.frp_subdomain)
            self.local_port.setValue(profile.runtime.local_port)
            self._set_combo_value(self.tool_profile, profile.runtime.tool_profile)
            self._set_combo_value(self.permission_mode, profile.runtime.permission_mode)
            self.runtime_command.setText(profile.runtime.runtime_command)
            self._set_combo_value(self.auth_type, profile.auth.type)
            self.oauth_client_id.setText(profile.auth.oauth_client_id)
            self.oauth_client_secret.setText(profile.auth.oauth_client_secret)
            self.oauth_password.setText(profile.auth.oauth_password)
            self.bearer_token.setText(profile.auth.bearer_token)

            self._set_combo_value(self.actions_tunnel_type, profile.actions.tunnel_type)
            self._set_combo_value(self.actions_cloudflare_mode, profile.actions.cloudflare_mode)
            self.actions_public_url_edit.setText(self._profile_actions_public_url_for_edit(profile))
            self.actions_cloudflare_token_edit.setText(profile.actions.cloudflare_token)
            self.actions_frp_server_edit.setText(profile.actions.frp_server)
            self.actions_subdomain_edit.setText(profile.actions.frp_subdomain)
            self.actions_local_port.setValue(profile.actions.local_port)
            self._set_combo_value(self.actions_permission_mode, profile.actions.permission_mode)
            self.actions_runtime_command.setText(profile.actions.runtime_command)
            self._set_combo_value(self.actions_auth_type, profile.actions.auth_type)
            self.actions_api_key.setText(profile.actions.api_key)
            self.actions_oauth_client_id.setText(profile.actions.oauth_client_id)
            self.actions_oauth_client_secret.setText(profile.actions.oauth_client_secret)
            self.actions_oauth_authorization_url.setText(profile.actions.oauth_authorization_url)
            self.actions_oauth_token_url.setText(profile.actions.oauth_token_url)
            self.actions_oauth_scopes.setText(profile.actions.oauth_scopes)
            self._set_combo_value(
                self.actions_oauth_token_exchange_method,
                profile.actions.oauth_token_exchange_method,
            )
            self.actions_allowed_commands.setText(profile.actions.allowed_commands)
            self.actions_max_patch_bytes.setValue(profile.actions.max_patch_bytes)
        finally:
            del blockers
            self._loading_profile = False

        self._render_status(self.mcp_runtime.status(profile), "mcp")
        self._render_status(self.actions_runtime.status(profile), "actions")
        self._load_logs(profile)
        self._refresh_mcp_tunnel_fields()
        self._refresh_mcp_auth_fields()
        self._refresh_actions_tunnel_fields()
        self._refresh_actions_auth_fields()
        self._refresh_connection_view()
        self._set_panel_enabled(True)
        self._update_header_for_active_tab()

    def _clear_panel(self) -> None:
        self._loading_profile = True
        blockers = [QSignalBlocker(widget) for widget in self._form_widgets()]
        try:
            self.header_title.setText("先添加一个工作区")
            self.header_meta.setText("左侧添加工作区后，再配置 MCP 或 Actions。")
            for widget in (
                self.name_edit,
                self.path_edit,
                self.public_url_edit,
                self.cloudflare_token_edit,
                self.frp_server_edit,
                self.subdomain_edit,
                self.oauth_client_id,
                self.oauth_client_secret,
                self.oauth_password,
                self.bearer_token,
                self.runtime_command,
                self.actions_public_url_edit,
                self.actions_cloudflare_token_edit,
                self.actions_frp_server_edit,
                self.actions_subdomain_edit,
                self.actions_runtime_command,
                self.actions_api_key,
                self.actions_oauth_client_id,
                self.actions_oauth_client_secret,
                self.actions_oauth_authorization_url,
                self.actions_oauth_token_url,
                self.actions_oauth_scopes,
            ):
                widget.clear()
            self.actions_allowed_commands.setText(
                "pytest,python,python3,npm,npx,node,pnpm,yarn,"
                "make,mvn,mvnw,gradle,gradlew,cargo,go,ruff,mypy,eslint,tsc"
            )
            self.local_port.setValue(28766)
            self.actions_local_port.setValue(8787)
            self.actions_max_patch_bytes.setValue(200000)
            self._set_combo_value(self.tunnel_type, "frp")
            self._set_combo_value(self.cloudflare_mode, "quick")
            self._set_combo_value(self.tool_profile, "full")
            self._set_combo_value(self.permission_mode, "trusted")
            self._set_combo_value(self.auth_type, "oauth")
            self._set_combo_value(self.actions_tunnel_type, "frp")
            self._set_combo_value(self.actions_cloudflare_mode, "quick")
            self._set_combo_value(self.actions_permission_mode, "trusted")
            self._set_combo_value(self.actions_auth_type, "api_key")
            self._set_combo_value(self.actions_oauth_token_exchange_method, "authorization_header")
        finally:
            del blockers
            self._loading_profile = False

        self.status_label.setText("未启动")
        self.status_label.setStyleSheet("font-weight:700; color:#b42318;")
        self.actions_status_label.setText("未启动")
        self.actions_status_label.setStyleSheet("font-weight:700; color:#b42318;")
        self.endpoint_label.setText("公网 MCP 地址：-")
        self.local_label.setText("本地 MCP 地址：-")
        self.endpoint_hint.setText("当前入口：-")
        self.actions_endpoint_hint.setText("OpenAPI：-")
        self.actions_openapi_label.setText("OpenAPI 地址：-")
        self.actions_privacy_label.setText("隐私政策地址：-")
        self.actions_local_label.setText("本地 Actions 地址：-")
        self.log_output.setPlainText("当前还没有日志。")
        self.actions_log_output.setPlainText("当前还没有日志。")
        self._refresh_mcp_tunnel_fields()
        self._refresh_mcp_auth_fields()
        self._refresh_actions_tunnel_fields()
        self._refresh_actions_auth_fields()
        self._set_panel_enabled(False)
        self._update_header_for_active_tab()

    def _set_panel_enabled(self, enabled: bool) -> None:
        for widget in (
            self.service_tabs,
            self.start_button,
            self.stop_button,
            self.copy_button,
            self.copy_frp_button,
            self.delete_button,
        ):
            widget.setEnabled(enabled)

    def _save_current(self) -> None:
        profile = self._require_profile()
        profile.name = self.name_edit.text().strip() or "工作区"
        profile.tunnel.type = self._combo_value(self.tunnel_type)
        profile.tunnel.cloudflare_mode = self._combo_value(self.cloudflare_mode)
        profile.tunnel.cloudflare_token = self.cloudflare_token_edit.text().strip()
        if profile.tunnel.type == "frp":
            profile.tunnel.public_url = self.public_url_edit.text().strip() or profile.tunnel.public_url
        elif profile.tunnel.cloudflare_mode == "named":
            profile.tunnel.public_url = self.public_url_edit.text().strip()
        else:
            profile.tunnel.public_url = ""
        profile.tunnel.frp_server = self.frp_server_edit.text().strip()
        profile.tunnel.frp_subdomain = self.subdomain_edit.text().strip()
        profile.runtime.local_port = self.local_port.value()
        profile.runtime.tool_profile = self._combo_value(self.tool_profile)
        profile.runtime.permission_mode = self._combo_value(self.permission_mode)
        profile.runtime.runtime_command = self.runtime_command.text().strip()
        profile.auth.type = self._combo_value(self.auth_type)
        profile.auth.oauth_client_id = self.oauth_client_id.text().strip() or profile.auth.oauth_client_id
        profile.auth.oauth_client_secret = self.oauth_client_secret.text().strip()
        profile.auth.oauth_password = self.oauth_password.text().strip() or profile.auth.oauth_password
        profile.auth.bearer_token = self.bearer_token.text().strip() or profile.auth.bearer_token

        profile.actions.tunnel_type = self._combo_value(self.actions_tunnel_type)
        profile.actions.cloudflare_mode = self._combo_value(self.actions_cloudflare_mode)
        profile.actions.cloudflare_token = self.actions_cloudflare_token_edit.text().strip()
        if profile.actions.tunnel_type == "frp":
            profile.actions.public_url = self.actions_public_url_edit.text().strip() or profile.actions.public_url
        elif profile.actions.cloudflare_mode == "named":
            profile.actions.public_url = self.actions_public_url_edit.text().strip()
        else:
            profile.actions.public_url = ""
        profile.actions.frp_server = self.actions_frp_server_edit.text().strip()
        profile.actions.frp_subdomain = self.actions_subdomain_edit.text().strip()
        profile.actions.local_port = self.actions_local_port.value()
        profile.actions.permission_mode = self._combo_value(self.actions_permission_mode)
        profile.actions.runtime_command = self.actions_runtime_command.text().strip()
        profile.actions.auth_type = self._combo_value(self.actions_auth_type)
        profile.actions.oauth_client_id = (
            self.actions_oauth_client_id.text().strip() or profile.actions.oauth_client_id
        )
        profile.actions.oauth_client_secret = (
            self.actions_oauth_client_secret.text().strip() or profile.actions.oauth_client_secret
        )
        profile.actions.oauth_authorization_url = self.actions_oauth_authorization_url.text().strip()
        profile.actions.oauth_token_url = self.actions_oauth_token_url.text().strip()
        profile.actions.oauth_scopes = self.actions_oauth_scopes.text().strip()
        profile.actions.oauth_token_exchange_method = self._combo_value(
            self.actions_oauth_token_exchange_method
        )
        profile.actions.allowed_commands = self.actions_allowed_commands.text().strip() or profile.actions.allowed_commands
        profile.actions.max_patch_bytes = self.actions_max_patch_bytes.value()
        profile.actions.api_key = self.actions_api_key.text().strip() or profile.actions.api_key

        save_profiles(self.profiles)
        self._populate_workspace_list()
        self._restore_selection(profile.id)

    def _add_workspace(self) -> None:
        directory = QFileDialog.getExistingDirectory(self, "选择工作区目录")
        if not directory:
            return
        profile = build_profile(directory)
        self.profiles.append(profile)
        save_profiles(self.profiles)
        self._populate_workspace_list()
        self.workspace_list.setCurrentRow(len(self.profiles) - 1)

    def _delete_workspace(self) -> None:
        profile = self._require_profile()
        answer = QMessageBox.question(
            self,
            "删除工作区",
            f"确定删除工作区“{profile.name}”吗？\n这不会删除磁盘目录，只会从客户端配置里移除。",
        )
        if answer != QMessageBox.StandardButton.Yes:
            return
        self.mcp_runtime.stop(profile)
        self.actions_runtime.stop(profile)
        current_index = self.workspace_list.currentRow()
        self.profiles = [item for item in self.profiles if item.id != profile.id]
        save_profiles(self.profiles)
        self.current_profile = None
        self._populate_workspace_list()
        if self.profiles:
            self.workspace_list.setCurrentRow(min(current_index, len(self.profiles) - 1))
        else:
            self._clear_panel()

    def _start_runtime(self) -> None:
        profile = self._require_profile()
        self._save_current()
        service = self._active_service()
        self._set_runtime_busy(True, "启动中", service)
        self._run_runtime_job(profile, "start", service)

    def _stop_runtime(self) -> None:
        profile = self._require_profile()
        service = self._active_service()
        self._set_runtime_busy(True, "停止中", service)
        self._run_runtime_job(profile, "stop", service)

    def _copy_endpoint(self) -> None:
        self._save_current()
        profile = self._require_profile()
        if self._active_service() == "actions":
            endpoint = self.actions_runtime.resolved_openapi_url(profile) or self._draft_actions_openapi_url()
            QApplication.clipboard().setText(endpoint)
            self.statusBar().showMessage("已复制 OpenAPI 地址到剪贴板", 3000)
            return
        endpoint = self.mcp_runtime.resolved_endpoint(profile) or self._draft_mcp_endpoint()
        QApplication.clipboard().setText(endpoint)
        self.statusBar().showMessage("已复制 MCP 地址到剪贴板", 3000)

    def _copy_actions_privacy_url(self) -> None:
        self._save_current()
        profile = self._require_profile()
        privacy_url = self.actions_runtime.resolved_privacy_url(profile) or self._draft_actions_privacy_url()
        QApplication.clipboard().setText(privacy_url)
        self.statusBar().showMessage("已复制隐私政策地址", 3000)

    def _copy_frp_snippet(self) -> None:
        self._save_current()
        profile = self._require_profile()
        snippet = profile.actions_frp_proxy_snippet() if self._active_service() == "actions" else profile.frp_proxy_snippet()
        QApplication.clipboard().setText(snippet)
        self.statusBar().showMessage("已复制 FRP 代理片段", 3000)

    def _copy_oauth_client_id(self) -> None:
        QApplication.clipboard().setText(self.oauth_client_id.text().strip())
        self.statusBar().showMessage("已复制 OAuth 客户端 ID", 3000)

    def _copy_oauth_client_secret(self) -> None:
        QApplication.clipboard().setText(self.oauth_client_secret.text().strip())
        self.statusBar().showMessage("已复制 OAuth 客户端密钥", 3000)

    def _copy_oauth_password(self) -> None:
        QApplication.clipboard().setText(self.oauth_password.text().strip())
        self.statusBar().showMessage("已复制授权口令", 3000)

    def _copy_bearer_token(self) -> None:
        QApplication.clipboard().setText(self.bearer_token.text().strip())
        self.statusBar().showMessage("已复制 Bearer Token", 3000)

    def _copy_actions_api_key(self) -> None:
        QApplication.clipboard().setText(self.actions_api_key.text().strip())
        self.statusBar().showMessage("已复制 Actions API Key", 3000)

    def _copy_actions_oauth_client_id(self) -> None:
        QApplication.clipboard().setText(self.actions_oauth_client_id.text().strip())
        self.statusBar().showMessage("已复制 Actions Client ID", 3000)

    def _copy_actions_oauth_client_secret(self) -> None:
        QApplication.clipboard().setText(self.actions_oauth_client_secret.text().strip())
        self.statusBar().showMessage("已复制 Actions Client Secret", 3000)

    def _copy_actions_oauth_authorization_url(self) -> None:
        QApplication.clipboard().setText(self.actions_oauth_authorization_url.text().strip())
        self.statusBar().showMessage("已复制授权 URL", 3000)

    def _copy_actions_oauth_token_url(self) -> None:
        QApplication.clipboard().setText(self.actions_oauth_token_url.text().strip())
        self.statusBar().showMessage("已复制令牌 URL", 3000)

    def _refresh_current(self) -> None:
        if self.current_profile is not None:
            self._load_profile(self.current_profile)

    def _refresh_mcp_tunnel_fields(self, *_args: object) -> None:
        if self._loading_profile:
            return
        tunnel_type = self._combo_value(self.tunnel_type)
        is_frp = tunnel_type == "frp"
        is_cloudflare = tunnel_type == "cloudflare"
        is_cloudflare_named = is_cloudflare and self._combo_value(self.cloudflare_mode) == "named"
        self._set_row_visible(self.cloudflare_mode_label, self.cloudflare_mode, is_cloudflare)
        self._set_row_visible(self.public_url_label, self.public_url_edit, is_cloudflare)
        self._set_row_visible(self.cloudflare_token_label, self.cloudflare_token_edit, is_cloudflare_named)
        self._set_row_visible(self.frp_server_label, self.frp_server_edit, is_frp)
        self._set_row_visible(self.subdomain_label, self.subdomain_edit, is_frp)
        self.public_url_edit.setReadOnly(is_cloudflare and not is_cloudflare_named)
        if is_cloudflare_named:
            self.public_url_edit.setPlaceholderText("例如：https://mcp.example.com")
        elif is_cloudflare:
            self.public_url_edit.setPlaceholderText("Cloudflare 启动后会自动分配公网地址")
        self._refresh_connection_view()

    def _refresh_actions_tunnel_fields(self, *_args: object) -> None:
        if self._loading_profile:
            return
        tunnel_type = self._combo_value(self.actions_tunnel_type)
        is_frp = tunnel_type == "frp"
        is_cloudflare = tunnel_type == "cloudflare"
        is_cloudflare_named = is_cloudflare and self._combo_value(self.actions_cloudflare_mode) == "named"
        self._set_row_visible(self.actions_cloudflare_mode_label, self.actions_cloudflare_mode, is_cloudflare)
        self._set_row_visible(self.actions_public_url_label, self.actions_public_url_edit, is_cloudflare)
        self._set_row_visible(self.actions_cloudflare_token_label, self.actions_cloudflare_token_edit, is_cloudflare_named)
        self._set_row_visible(self.actions_frp_server_label, self.actions_frp_server_edit, is_frp)
        self._set_row_visible(self.actions_subdomain_label, self.actions_subdomain_edit, is_frp)
        self.actions_public_url_edit.setReadOnly(is_cloudflare and not is_cloudflare_named)
        if is_cloudflare_named:
            self.actions_public_url_edit.setPlaceholderText("例如：https://actions.example.com")
        elif is_cloudflare:
            self.actions_public_url_edit.setPlaceholderText("Cloudflare 启动后会自动分配公网地址")
        self._refresh_connection_view()

    def _refresh_mcp_auth_fields(self, *_args: object) -> None:
        if self._loading_profile:
            return
        auth_type = self._combo_value(self.auth_type)
        is_oauth = auth_type == "oauth"
        is_bearer = auth_type == "bearer"
        self._set_row_visible(self.oauth_client_id_label, self.oauth_client_id, is_oauth)
        self._set_row_visible(self.oauth_client_secret_label, self.oauth_client_secret, is_oauth)
        self._set_row_visible(self.oauth_password_label, self.oauth_password, is_oauth)
        self._set_row_visible(self.bearer_token_label, self.bearer_token, is_bearer)
        self.oauth_actions.setVisible(is_oauth)
        self.bearer_actions.setVisible(is_bearer)
        if is_oauth:
            self.auth_hint.setText("ChatGPT 里填写 OAuth 客户端 ID 和 OAuth 客户端密钥；首次授权时再输入授权口令。")
        elif is_bearer:
            self.auth_hint.setText("Bearer 模式下，把这个 Token 配给调用方即可。")
        else:
            self.auth_hint.setText("当前不会要求认证，适合纯本地调试，不建议直接暴露到公网。")
        self._refresh_connection_view()

    def _refresh_actions_auth_fields(self, *_args: object) -> None:
        if self._loading_profile:
            return
        auth_type = self._combo_value(self.actions_auth_type)
        is_api_key = auth_type == "api_key"
        is_oauth = auth_type == "oauth"
        self._set_row_visible(self.actions_api_key_label, self.actions_api_key, is_api_key)
        self._set_row_visible(self.actions_oauth_client_id_label, self.actions_oauth_client_id, is_oauth)
        self._set_row_visible(
            self.actions_oauth_client_secret_label,
            self.actions_oauth_client_secret,
            is_oauth,
        )
        self._set_row_visible(
            self.actions_oauth_authorization_url_label,
            self.actions_oauth_authorization_url,
            is_oauth,
        )
        self._set_row_visible(self.actions_oauth_token_url_label, self.actions_oauth_token_url, is_oauth)
        self._set_row_visible(self.actions_oauth_scopes_label, self.actions_oauth_scopes, is_oauth)
        self._set_row_visible(
            self.actions_oauth_token_exchange_method_label,
            self.actions_oauth_token_exchange_method,
            is_oauth,
        )
        self.actions_api_key_actions.setVisible(is_api_key)
        self.actions_oauth_actions.setVisible(is_oauth)
        if is_api_key:
            self.actions_auth_hint.setText(
                "在私有 GPT 的 Actions 里选择 API Key，认证类型选 Bearer，Key 直接填这里的 API Key。"
            )
        elif is_oauth:
            self.actions_auth_hint.setText(
                "这些 OAuth 字段会保存到本地，方便你整理 GPT 表单；当前 Actions gateway 还未接入 OAuth。"
            )
        else:
            self.actions_auth_hint.setText("当前不校验认证，只建议在本机或受保护的内网环境使用。")

    def _refresh_connection_view(self, *_args: object) -> None:
        if self._loading_profile:
            return
        mcp_endpoint = self._draft_mcp_endpoint()
        self.endpoint_label.setText(f"公网 MCP 地址：{mcp_endpoint}")
        self.local_label.setText(f"本地 MCP 地址：http://127.0.0.1:{self.local_port.value()}/mcp")
        self.endpoint_hint.setText(f"当前入口：{mcp_endpoint}")

        actions_openapi = self._draft_actions_openapi_url()
        actions_privacy = self._draft_actions_privacy_url()
        self.actions_openapi_label.setText(f"OpenAPI 地址：{actions_openapi}")
        self.actions_privacy_label.setText(f"隐私政策地址：{actions_privacy}")
        self.actions_local_label.setText(f"本地 Actions 地址：http://127.0.0.1:{self.actions_local_port.value()}")
        self.actions_endpoint_hint.setText(f"OpenAPI：{actions_openapi}")

    def _load_logs(self, profile: WorkspaceProfile) -> None:
        log_dir = log_dir_for_profile(profile.id)
        mcp_output: list[str] = []
        mcp_log_names = ["stderr.log", "stdout.log"]
        if profile.tunnel.type == "cloudflare":
            mcp_log_names.insert(0, "cloudflared.log")
        for name in mcp_log_names:
            path = log_dir / name
            if path.exists():
                mcp_output.append(f"[{name}]\n{self._read_log_tail(path)}")
        self.log_output.setPlainText("\n\n".join(mcp_output) if mcp_output else "当前还没有日志。")

        actions_output: list[str] = []
        actions_log_names = ["actions-stderr.log", "actions-stdout.log"]
        if profile.actions.tunnel_type == "cloudflare":
            actions_log_names.insert(0, "actions-cloudflared.log")
        for name in actions_log_names:
            path = log_dir / name
            if path.exists():
                actions_output.append(f"[{name}]\n{self._read_log_tail(path)}")
        self.actions_log_output.setPlainText("\n\n".join(actions_output) if actions_output else "当前还没有日志。")

    def _render_status(self, status: RuntimeStatus, service: str) -> None:
        state_map = {
            "running": "运行中",
            "stopped": "已停止",
            "starting": "启动中",
            "error": "异常",
            "stopping": "停止中",
        }
        label = self.status_label if service == "mcp" else self.actions_status_label
        state_text = state_map.get(status.state, status.state)
        color = "#067647" if status.state == "running" else "#b42318"
        label.setText(f"{state_text}  PID={status.pid or '-'}")
        label.setStyleSheet(f"font-weight:700; color:{color};")

    def _run_runtime_job(self, profile: WorkspaceProfile, action: str, service: str) -> None:
        if self._runtime_thread is not None:
            return
        runtime = self.mcp_runtime if service == "mcp" else self.actions_runtime
        self._runtime_thread = QThread(self)
        self._runtime_job = RuntimeJob(runtime, profile, action)
        self._runtime_job.moveToThread(self._runtime_thread)
        self._runtime_thread.started.connect(self._runtime_job.run)
        self._runtime_job.finished.connect(self._on_runtime_job_finished)
        self._runtime_job.finished.connect(self._runtime_thread.quit)
        self._runtime_thread.finished.connect(self._cleanup_runtime_job)
        self._runtime_thread.start()

    def _on_runtime_job_finished(self, action: str, status: object, error_message: str) -> None:
        profile = self.current_profile
        service = self._busy_service or self._active_service()
        self._set_runtime_busy(False, service=service)
        if profile is None:
            return
        if error_message:
            self._load_logs(profile)
            self._refresh_workspace_item(profile.id)
            QMessageBox.critical(self, "启动失败" if action == "start" else "停止失败", error_message)
            return
        if isinstance(status, RuntimeStatus):
            self._render_status(status, service)
        self._sync_profile_runtime_view(profile, service)
        self._load_logs(profile)
        self._refresh_workspace_item(profile.id)

    def _cleanup_runtime_job(self) -> None:
        if self._runtime_job is not None:
            self._runtime_job.deleteLater()
            self._runtime_job = None
        if self._runtime_thread is not None:
            self._runtime_thread.deleteLater()
            self._runtime_thread = None

    def _set_runtime_busy(self, busy: bool, state_text: str | None = None, service: str | None = None) -> None:
        active_service = service or self._active_service()
        self.start_button.setEnabled(not busy and self.current_profile is not None)
        self.stop_button.setEnabled(not busy and self.current_profile is not None)
        self.workspace_list.setEnabled(not busy)
        self.service_tabs.setEnabled(not busy and self.current_profile is not None)
        if busy:
            profile = self.current_profile
            self._busy_profile_id = profile.id if profile is not None else None
            self._busy_action = state_text
            self._busy_service = active_service
            self._busy_dots = 0
            self.start_button.setText("启动中..." if state_text == "启动中" else "启动")
            self.stop_button.setText("停止中..." if state_text == "停止中" else "停止")
            label = self.status_label if active_service == "mcp" else self.actions_status_label
            if state_text:
                label.setText(f"{state_text}  PID=-")
                label.setStyleSheet("font-weight:700; color:#b54708;")
            self.statusBar().showMessage(f"{state_text}，请稍候...", 0)
            if not self._busy_timer.isActive():
                self._busy_timer.start()
            if self._busy_profile_id:
                self._refresh_workspace_item(self._busy_profile_id)
            return
        self._busy_timer.stop()
        self._busy_profile_id = None
        self._busy_action = None
        self._busy_service = None
        self._busy_dots = 0
        self.start_button.setText("启动")
        self.stop_button.setText("停止")
        self.statusBar().clearMessage()
        self._update_header_for_active_tab()

    def _tick_busy_indicator(self) -> None:
        if self._busy_action is None or self._busy_service is None:
            return
        self._busy_dots = (self._busy_dots + 1) % 4
        dots = "." * self._busy_dots
        label = self.status_label if self._busy_service == "mcp" else self.actions_status_label
        label.setText(f"{self._busy_action}{dots}  PID=-")
        label.setStyleSheet("font-weight:700; color:#b54708;")
        if self._busy_profile_id:
            self._refresh_workspace_item(self._busy_profile_id)

    def _sync_profile_runtime_view(self, profile: WorkspaceProfile, service: str) -> None:
        if service == "mcp":
            if profile.tunnel.type == "cloudflare":
                public_url = self.mcp_runtime.resolved_public_url(profile)
                if public_url:
                    self.public_url_edit.setText(public_url)
                elif profile.tunnel.cloudflare_mode != "named":
                    self.public_url_edit.clear()
        else:
            if profile.actions.tunnel_type == "cloudflare":
                public_url = self.actions_runtime.resolved_public_url(profile)
                if public_url:
                    self.actions_public_url_edit.setText(public_url)
                elif profile.actions.cloudflare_mode != "named":
                    self.actions_public_url_edit.clear()
        self._refresh_connection_view()

    def _refresh_workspace_item(self, profile_id: str) -> None:
        for index, profile in enumerate(self.profiles):
            if profile.id == profile_id:
                item = self.workspace_list.item(index)
                if item is not None:
                    item.setText(self._workspace_summary(profile))
                break

    def _draft_mcp_public_url(self) -> str:
        tunnel_type = self._combo_value(self.tunnel_type)
        if tunnel_type == "frp":
            subdomain = self.subdomain_edit.text().strip()
            server = self.frp_server_edit.text().strip()
            if subdomain and server:
                return f"https://{subdomain}.{server}"
        if tunnel_type == "cloudflare":
            if self.current_profile is not None:
                resolved = self.mcp_runtime.resolved_public_url(self.current_profile)
                if resolved:
                    return resolved
            if self._combo_value(self.cloudflare_mode) == "named":
                return self.public_url_edit.text().strip().rstrip("/")
            return ""
        return self.public_url_edit.text().strip().rstrip("/")

    def _draft_mcp_endpoint(self) -> str:
        base_url = self._draft_mcp_public_url().rstrip("/")
        return f"{base_url}/mcp" if base_url else "-"

    def _draft_actions_public_url(self) -> str:
        tunnel_type = self._combo_value(self.actions_tunnel_type)
        if tunnel_type == "frp":
            subdomain = self.actions_subdomain_edit.text().strip()
            server = self.actions_frp_server_edit.text().strip()
            if subdomain and server:
                return f"https://{subdomain}.{server}"
        if tunnel_type == "cloudflare":
            if self.current_profile is not None:
                resolved = self.actions_runtime.resolved_public_url(self.current_profile)
                if resolved:
                    return resolved
            if self._combo_value(self.actions_cloudflare_mode) == "named":
                return self.actions_public_url_edit.text().strip().rstrip("/")
            return ""
        return self.actions_public_url_edit.text().strip().rstrip("/")

    def _draft_actions_openapi_url(self) -> str:
        base_url = self._draft_actions_public_url().rstrip("/")
        return f"{base_url}/openapi.json" if base_url else "-"

    def _draft_actions_privacy_url(self) -> str:
        base_url = self._draft_actions_public_url().rstrip("/")
        return f"{base_url}/privacy" if base_url else "-"

    def _workspace_summary(self, profile: WorkspaceProfile) -> str:
        mcp_state = self._workspace_state(profile, "mcp")
        actions_state = self._workspace_state(profile, "actions")
        state_map = {
            "running": "运行中",
            "stopped": "已停止",
            "starting": "启动中",
            "error": "异常",
            "stopping": "停止中",
        }
        mcp_endpoint = self.mcp_runtime.resolved_endpoint(profile) or profile.endpoint or "-"
        actions_endpoint = self.actions_runtime.resolved_openapi_url(profile) or profile.actions_openapi_url or "-"
        return "\n".join(
            [
                profile.name,
                profile.path,
                f"MCP：{state_map.get(mcp_state, mcp_state)}  Actions：{state_map.get(actions_state, actions_state)}",
                f"MCP：{mcp_endpoint} | Actions：{actions_endpoint}",
            ]
        )

    def _restore_selection(self, profile_id: str) -> None:
        for index, profile in enumerate(self.profiles):
            if profile.id == profile_id:
                self.workspace_list.setCurrentRow(index)
                return

    def _require_profile(self) -> WorkspaceProfile:
        if self.current_profile is None:
            raise RuntimeError("当前没有选中工作区。")
        return self.current_profile

    def _fill_combo(self, combo: QComboBox, options: list[tuple[str, str]]) -> None:
        for value, label in options:
            combo.addItem(label, value)

    def _combo_value(self, combo: QComboBox) -> str:
        return str(combo.currentData())

    def _set_combo_value(self, combo: QComboBox, value: str) -> None:
        index = combo.findData(value)
        if index >= 0:
            combo.setCurrentIndex(index)

    def _set_row_visible(self, label: QLabel, field: QWidget, visible: bool) -> None:
        label.setVisible(visible)
        field.setVisible(visible)

    def _profile_public_url_for_edit(self, profile: WorkspaceProfile) -> str:
        if profile.tunnel.type == "frp":
            return profile.tunnel.public_url
        resolved = self.mcp_runtime.resolved_public_url(profile)
        if resolved:
            return resolved
        if profile.tunnel.cloudflare_mode == "named":
            return profile.tunnel.public_url
        return ""

    def _profile_actions_public_url_for_edit(self, profile: WorkspaceProfile) -> str:
        if profile.actions.tunnel_type == "frp":
            return profile.actions.public_url
        resolved = self.actions_runtime.resolved_public_url(profile)
        if resolved:
            return resolved
        if profile.actions.cloudflare_mode == "named":
            return profile.actions.public_url
        return ""

    def _workspace_state(self, profile: WorkspaceProfile, service: str) -> str:
        if self._busy_profile_id == profile.id and self._busy_action == "启动中" and self._busy_service == service:
            return "starting"
        if self._busy_profile_id == profile.id and self._busy_action == "停止中" and self._busy_service == service:
            return "stopping"
        if service == "mcp":
            return self.mcp_runtime.summary_state(profile)
        return self.actions_runtime.summary_state(profile)

    def _active_service(self) -> str:
        return "actions" if self.service_tabs.currentIndex() == 1 else "mcp"

    def _update_header_for_active_tab(self) -> None:
        if self._active_service() == "actions":
            self.copy_button.setText("复制 OpenAPI 地址")
        else:
            self.copy_button.setText("复制 MCP 地址")

    def _read_log_tail(self, path: Path, max_bytes: int = 8192) -> str:
        with path.open("rb") as handle:
            handle.seek(0, 2)
            size = handle.tell()
            handle.seek(max(size - max_bytes, 0))
            data = handle.read()
        return data.decode("utf-8", errors="replace")[-4000:]

    def _form_widgets(self) -> tuple[QWidget, ...]:
        return (
            self.name_edit,
            self.path_edit,
            self.tunnel_type,
            self.cloudflare_mode,
            self.public_url_edit,
            self.cloudflare_token_edit,
            self.frp_server_edit,
            self.subdomain_edit,
            self.local_port,
            self.tool_profile,
            self.permission_mode,
            self.runtime_command,
            self.auth_type,
            self.oauth_client_id,
            self.oauth_client_secret,
            self.oauth_password,
            self.bearer_token,
            self.actions_tunnel_type,
            self.actions_cloudflare_mode,
            self.actions_public_url_edit,
            self.actions_cloudflare_token_edit,
            self.actions_frp_server_edit,
            self.actions_subdomain_edit,
            self.actions_local_port,
            self.actions_permission_mode,
            self.actions_runtime_command,
            self.actions_auth_type,
            self.actions_api_key,
            self.actions_oauth_client_id,
            self.actions_oauth_client_secret,
            self.actions_oauth_authorization_url,
            self.actions_oauth_token_url,
            self.actions_oauth_scopes,
            self.actions_oauth_token_exchange_method,
            self.actions_allowed_commands,
            self.actions_max_patch_bytes,
        )


def main() -> int:
    app = QApplication(sys.argv)
    app.setStyleSheet(STYLESHEET)
    window = MainWindow()
    window.show()

    def _present_window() -> None:
        screen = app.primaryScreen()
        if screen is not None:
            available = screen.availableGeometry()
            frame = window.frameGeometry()
            frame.moveCenter(available.center())
            window.move(frame.topLeft())
        window.setWindowState((window.windowState() & ~Qt.WindowState.WindowMinimized) | Qt.WindowState.WindowActive)
        window.raise_()
        window.activateWindow()

    QTimer.singleShot(0, _present_window)
    return app.exec()
