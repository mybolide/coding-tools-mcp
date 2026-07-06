const JSON_CONTENT_TYPE = "application/json; charset=utf-8";
const DEFAULT_WORKFLOW_ID = "start-sandbox.yml";
const DEFAULT_IMAGE = "ghcr.io/xytom/coding-tools-mcp-sandbox:latest";
const DEFAULT_PORT = "8765";
const DEFAULT_DURATION_MINUTES = "120";
const DEFAULT_PERMISSION_MODE = "trusted";
const DEFAULT_TOOL_PROFILE = "full";
const DEFAULT_TUNNEL_TYPE = "named";

const PERMISSION_MODES = new Set(["safe", "trusted", "dangerous"]);
const TOOL_PROFILES = new Set(["full", "read-only", "compat-readonly-all"]);
const TUNNEL_TYPES = new Set(["quick", "named"]);

const START_TOOL = {
  name: "start_coding_tools_sandbox",
  description: "Trigger the GitHub Actions workflow that starts a coding-tools-mcp Docker sandbox and exposes it through Cloudflare Tunnel.",
  inputSchema: {
    type: "object",
    properties: {
      ref: { type: "string", description: "Git ref to run the workflow on. Defaults to GITHUB_REF." },
      duration_minutes: { type: "string", description: "Sandbox lifetime, 5-330 minutes." },
      permission_mode: { type: "string", enum: [...PERMISSION_MODES] },
      tool_profile: { type: "string", enum: [...TOOL_PROFILES] },
      tunnel_type: { type: "string", enum: [...TUNNEL_TYPES] },
      tunnel_hostname: { type: "string", description: "Stable hostname for tunnel_type=named, for example mcp.example.com." },
      checkout_repository: { type: "boolean" },
    },
    additionalProperties: false,
  },
};

export default {
  async fetch(request, env) {
    try {
      const url = new URL(request.url);

      if (request.method === "GET" && url.pathname === "/") {
        return jsonResponse({ ok: true, service: "coding-tools-sandbox-control" });
      }

      const auth = await requireBearerAuth(request, env.CONTROL_TOKEN);
      if (auth) return auth;

      if (url.pathname === "/start") {
        if (request.method !== "POST") return methodNotAllowed("POST");
        const body = await readJsonBody(request);
        const result = await startSandbox(body ?? {}, env);
        return jsonResponse(result, { status: 202 });
      }

      if (url.pathname === "/mcp") {
        if (request.method !== "POST") return methodNotAllowed("POST");
        const body = await readJsonBody(request);
        return handleMcpRequest(body, env);
      }

      return jsonResponse({ error: "not_found" }, { status: 404 });
    } catch (error) {
      const status = error instanceof HttpError ? error.status : 500;
      const code = error instanceof HttpError ? error.code : "internal_error";
      return jsonResponse({ error: code, message: error.message }, { status });
    }
  },
};

async function handleMcpRequest(body, env) {
  if (!body || typeof body !== "object") {
    throw new HttpError(400, "invalid_json_rpc", "Expected a JSON-RPC request body.");
  }

  const id = body.id ?? null;
  if (body.jsonrpc !== "2.0" || typeof body.method !== "string") {
    return jsonRpcError(id, -32600, "Invalid Request");
  }

  if (id === null && body.method.startsWith("notifications/")) {
    return new Response(null, { status: 202 });
  }

  if (body.method === "initialize") {
    return jsonRpcResult(id, {
      protocolVersion: "2025-06-18",
      capabilities: { tools: {} },
      serverInfo: { name: "coding-tools-sandbox-control", version: "0.1.0" },
    });
  }

  if (body.method === "ping") {
    return jsonRpcResult(id, {});
  }

  if (body.method === "resources/list") {
    return jsonRpcResult(id, { resources: [] });
  }

  if (body.method === "prompts/list") {
    return jsonRpcResult(id, { prompts: [] });
  }

  if (body.method === "tools/list") {
    return jsonRpcResult(id, { tools: [START_TOOL] });
  }

  if (body.method === "tools/call") {
    const name = body.params?.name;
    if (name !== START_TOOL.name) {
      return jsonRpcError(id, -32602, `Unknown tool: ${String(name)}`);
    }

    try {
      const result = await startSandbox(body.params?.arguments ?? {}, env);
      return jsonRpcResult(id, {
        content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
        structuredContent: result,
      });
    } catch (error) {
      return jsonRpcResult(id, {
        isError: true,
        content: [{ type: "text", text: error.message }],
      });
    }
  }

  return jsonRpcError(id, -32601, "Method not found");
}

async function startSandbox(input, env) {
  const owner = requiredEnv(env, "GITHUB_OWNER");
  const repo = requiredEnv(env, "GITHUB_REPO");
  const githubToken = requiredEnv(env, "GITHUB_TOKEN");
  const ref = cleanRef(input.ref ?? env.GITHUB_REF ?? "main");
  const workflowId = cleanWorkflowId(input.workflow_id ?? env.WORKFLOW_ID ?? DEFAULT_WORKFLOW_ID);
  const inputs = buildWorkflowInputs(input, env);
  const mcpUrl = inputs.tunnel_type === "named" ? `https://${inputs.tunnel_hostname}/mcp` : null;

  const endpoint = `https://api.github.com/repos/${encodeURIComponent(owner)}/${encodeURIComponent(repo)}/actions/workflows/${encodeURIComponent(workflowId)}/dispatches`;
  const response = await fetch(endpoint, {
    method: "POST",
    headers: {
      Accept: "application/vnd.github+json",
      Authorization: `Bearer ${githubToken}`,
      "Content-Type": "application/json",
      "User-Agent": "coding-tools-sandbox-control-worker",
      "X-GitHub-Api-Version": "2026-03-10",
    },
    body: JSON.stringify({ ref, inputs }),
  });

  let workflowRun = null;
  if (response.status === 200) {
    workflowRun = await response.json().catch(() => null);
  } else if (response.status !== 204) {
    const text = await response.text();
    throw new HttpError(response.status, "github_dispatch_failed", formatGitHubDispatchError(text, ref, workflowId, response.status));
  }

  return {
    ok: true,
    github_status: response.status,
    owner,
    repo,
    workflow_id: workflowId,
    ref,
    inputs,
    mcp_url: mcpUrl,
    mcp_authorization: mcpUrl ? "Bearer <CODING_TOOLS_MCP_AUTH_TOKEN>" : null,
    workflow_run: workflowRun,
    message: "Sandbox workflow dispatch accepted by GitHub.",
  };
}

function formatGitHubDispatchError(text, ref, workflowId, status) {
  if (!text) return `GitHub returned HTTP ${status}`;

  let parsed = null;
  try {
    parsed = JSON.parse(text);
  } catch {
    return text;
  }

  const message = parsed?.message ? String(parsed.message) : text;
  if (status === 422 && message.includes("Unexpected inputs")) {
    return `${message}. The workflow '${workflowId}' at ref '${ref}' does not declare one or more inputs sent by this Worker. Merge the updated workflow into that ref, set GITHUB_REF to a branch that has it, or pass a matching 'ref' in the request.`;
  }

  return text;
}

function buildWorkflowInputs(input, env) {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    throw new HttpError(400, "invalid_input", "Request body must be a JSON object.");
  }

  const permissionMode = cleanEnum(input.permission_mode ?? env.DEFAULT_PERMISSION_MODE ?? DEFAULT_PERMISSION_MODE, PERMISSION_MODES, "permission_mode");
  if (permissionMode === "dangerous" && env.ALLOW_DANGEROUS !== "true") {
    throw new HttpError(400, "dangerous_mode_disabled", "Set ALLOW_DANGEROUS=true to allow permission_mode=dangerous.");
  }

  const tunnelType = cleanEnum(input.tunnel_type ?? env.DEFAULT_TUNNEL_TYPE ?? DEFAULT_TUNNEL_TYPE, TUNNEL_TYPES, "tunnel_type");
  const tunnelHostname = cleanHostname(input.tunnel_hostname ?? env.TUNNEL_HOSTNAME ?? "", tunnelType);
  const defaultImage = env.DEFAULT_IMAGE ?? DEFAULT_IMAGE;
  const image = cleanImage(input.image ?? defaultImage, defaultImage, env.ALLOW_IMAGE_OVERRIDE === "true");

  return {
    image,
    port: cleanPort(input.port ?? env.DEFAULT_PORT ?? DEFAULT_PORT),
    permission_mode: permissionMode,
    tool_profile: cleanEnum(input.tool_profile ?? env.DEFAULT_TOOL_PROFILE ?? DEFAULT_TOOL_PROFILE, TOOL_PROFILES, "tool_profile"),
    checkout_repository: cleanBoolean(input.checkout_repository ?? true, "checkout_repository"),
    duration_minutes: cleanDuration(input.duration_minutes ?? env.DEFAULT_DURATION_MINUTES ?? DEFAULT_DURATION_MINUTES),
    auth_token: "",
    hide_auth_token: true,
    tunnel_type: tunnelType,
    tunnel_hostname: tunnelHostname,
  };
}

async function requireBearerAuth(request, expectedToken) {
  if (!expectedToken) {
    return jsonResponse({ error: "server_misconfigured", message: "CONTROL_TOKEN secret is not configured." }, { status: 500 });
  }

  const header = request.headers.get("Authorization") ?? "";
  const match = header.match(/^Bearer\s+(.+)$/i);
  if (!match || !(await secureEqual(match[1], expectedToken))) {
    return jsonResponse({ error: "unauthorized" }, { status: 401, headers: { "WWW-Authenticate": "Bearer" } });
  }
  return null;
}

async function secureEqual(a, b) {
  const encoder = new TextEncoder();
  const [left, right] = await Promise.all([
    crypto.subtle.digest("SHA-256", encoder.encode(String(a))),
    crypto.subtle.digest("SHA-256", encoder.encode(String(b))),
  ]);
  const leftBytes = new Uint8Array(left);
  const rightBytes = new Uint8Array(right);
  let diff = leftBytes.length ^ rightBytes.length;
  for (let i = 0; i < Math.max(leftBytes.length, rightBytes.length); i += 1) {
    diff |= (leftBytes[i] ?? 0) ^ (rightBytes[i] ?? 0);
  }
  return diff === 0;
}

async function readJsonBody(request) {
  const text = await request.text();
  if (!text.trim()) return {};
  try {
    return JSON.parse(text);
  } catch {
    throw new HttpError(400, "invalid_json", "Request body must be valid JSON.");
  }
}

function cleanDuration(value) {
  const duration = cleanString(value, "duration_minutes");
  if (!/^\d+$/.test(duration)) throw new HttpError(400, "invalid_duration", "duration_minutes must be an integer string.");
  const numeric = Number(duration);
  if (numeric < 5 || numeric > 330) throw new HttpError(400, "invalid_duration", "duration_minutes must be between 5 and 330.");
  return duration;
}

function cleanPort(value) {
  const port = cleanString(value, "port");
  if (!/^\d+$/.test(port)) throw new HttpError(400, "invalid_port", "port must be an integer string.");
  const numeric = Number(port);
  if (numeric < 1 || numeric > 65535) throw new HttpError(400, "invalid_port", "port must be between 1 and 65535.");
  return port;
}

function cleanEnum(value, allowed, name) {
  const text = cleanString(value, name);
  if (!allowed.has(text)) throw new HttpError(400, `invalid_${name}`, `${name} must be one of: ${[...allowed].join(", ")}.`);
  return text;
}

function cleanHostname(value, tunnelType) {
  const hostname = String(value ?? "").replace(/^https?:\/\//, "").split("/")[0].trim();
  if (tunnelType !== "named") return "";
  if (!hostname) throw new HttpError(400, "missing_tunnel_hostname", "tunnel_hostname is required when tunnel_type=named.");
  if (!/^[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?(\.[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?)+$/.test(hostname)) {
    throw new HttpError(400, "invalid_tunnel_hostname", "tunnel_hostname must be a valid hostname, for example mcp.example.com.");
  }
  return hostname;
}

function cleanImage(value, defaultImage, allowOverride) {
  const image = cleanString(value, "image");
  if (!allowOverride && image !== defaultImage) {
    throw new HttpError(400, "image_override_disabled", "Set ALLOW_IMAGE_OVERRIDE=true to dispatch a non-default image.");
  }
  if (!/^[A-Za-z0-9][A-Za-z0-9._/:@+-]{0,255}$/.test(image)) {
    throw new HttpError(400, "invalid_image", "image contains unsupported characters.");
  }
  return image;
}

function cleanRef(value) {
  const ref = cleanString(value, "ref");
  if (!/^[A-Za-z0-9._/@+-]{1,255}$/.test(ref)) throw new HttpError(400, "invalid_ref", "ref contains unsupported characters.");
  return ref;
}

function cleanWorkflowId(value) {
  const workflowId = cleanString(value, "workflow_id");
  if (!/^[A-Za-z0-9._-]+\.(ya?ml)$/.test(workflowId)) {
    throw new HttpError(400, "invalid_workflow_id", "workflow_id must be a workflow YAML filename, for example start-sandbox.yml.");
  }
  return workflowId;
}

function cleanBoolean(value, name) {
  if (typeof value === "boolean") return value;
  if (value === "true") return true;
  if (value === "false") return false;
  throw new HttpError(400, `invalid_${name}`, `${name} must be a boolean.`);
}

function cleanString(value, name) {
  if (typeof value !== "string") throw new HttpError(400, `invalid_${name}`, `${name} must be a string.`);
  const text = value.trim();
  if (!text) throw new HttpError(400, `missing_${name}`, `${name} is required.`);
  return text;
}

function requiredEnv(env, name) {
  const value = env[name];
  if (!value) throw new HttpError(500, "server_misconfigured", `${name} is not configured.`);
  return value;
}

function methodNotAllowed(method) {
  return jsonResponse({ error: "method_not_allowed" }, { status: 405, headers: { Allow: method } });
}

function jsonRpcResult(id, result) {
  return jsonResponse({ jsonrpc: "2.0", id, result });
}

function jsonRpcError(id, code, message) {
  return jsonResponse({ jsonrpc: "2.0", id, error: { code, message } });
}

function jsonResponse(body, init = {}) {
  const headers = new Headers(init.headers ?? {});
  headers.set("Content-Type", JSON_CONTENT_TYPE);
  headers.set("Cache-Control", "no-store");
  return new Response(JSON.stringify(body), { ...init, headers });
}

class HttpError extends Error {
  constructor(status, code, message) {
    super(message);
    this.name = "HttpError";
    this.status = status;
    this.code = code;
  }
}
