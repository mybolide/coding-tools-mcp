#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

PYTHON="${PYTHON:-python3}"
MODE="testpypi"
RUN_TESTS=1
VERIFY_INSTALL=1
ASSUME_YES=0
ALLOW_DIRTY=0

usage() {
  cat <<'EOF'
Usage: scripts/publish-pypi.sh [--testpypi|--pypi|--both] [options]

Build, check, upload, and verify the Python package release.

Modes:
  --testpypi             Upload to TestPyPI only. This is the default.
  --pypi                 Upload to production PyPI only.
  --both                 Upload to TestPyPI, verify it, then upload to PyPI.

Options:
  --skip-tests           Do not run make test before building.
  --skip-verify-install  Do not install the uploaded package for verification.
  --allow-dirty          Allow tracked git changes in the release checkout.
  -y, --yes              Skip the production PyPI confirmation prompt.
  -h, --help             Show this help.

Prerequisites:
  python -m pip install --user build twine

Credentials:
  Use TWINE_USERNAME/TWINE_PASSWORD or ~/.pypirc.
  For token auth, username should be __token__ and password should be the
  PyPI or TestPyPI API token.

Examples:
  scripts/publish-pypi.sh
  scripts/publish-pypi.sh --both
  scripts/publish-pypi.sh --pypi --skip-tests
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

log() {
  echo "==> $*" >&2
}

run() {
  echo "+ $*" >&2
  "$@"
}

pyproject_value() {
  local key="$1"
  "$PYTHON" -c 'import pathlib, sys, tomllib; data = tomllib.loads(pathlib.Path("pyproject.toml").read_text()); print(data["project"][sys.argv[1]])' "$key"
}

project_script_name() {
  "$PYTHON" -c 'import pathlib, tomllib; data = tomllib.loads(pathlib.Path("pyproject.toml").read_text()); print(next(iter(data["project"]["scripts"])))'
}

ensure_module() {
  local module="$1"
  local package="$2"
  if "$PYTHON" -c "import $module" >/dev/null 2>&1; then
    return
  fi
  log "Installing missing Python package: $package"
  run "$PYTHON" -m pip install --user "$package"
}

check_git_clean() {
  if [[ "$ALLOW_DIRTY" == "1" ]]; then
    return
  fi
  if ! command -v git >/dev/null 2>&1; then
    return
  fi
  if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    return
  fi
  local dirty
  dirty="$(git status --porcelain --untracked-files=no)"
  if [[ -n "$dirty" ]]; then
    echo "$dirty" >&2
    die "tracked git changes are present; commit them or pass --allow-dirty"
  fi
}

check_credentials() {
  local repository="$1"
  "$PYTHON" - "$repository" <<'PY'
import configparser
import os
import pathlib
import sys

repository = sys.argv[1]
if os.environ.get("TWINE_USERNAME") and os.environ.get("TWINE_PASSWORD"):
    raise SystemExit(0)

pypirc = pathlib.Path.home() / ".pypirc"
if not pypirc.exists():
    raise SystemExit(f"missing {pypirc}; set TWINE_USERNAME/TWINE_PASSWORD or create ~/.pypirc")

config = configparser.RawConfigParser()
config.read(pypirc)
username = config.get(repository, "username", fallback="")
password = config.get(repository, "password", fallback="")
if not username or not password:
    raise SystemExit(f"missing username/password for [{repository}] in {pypirc}")
if "EOF" in password or "your-" in password or "\u4f60\u7684" in password:
    raise SystemExit(f"[{repository}] token looks like a placeholder or malformed heredoc output")
PY
}

confirm_production_upload() {
  local project_name="$1"
  local project_version="$2"
  if [[ "$ASSUME_YES" == "1" ]]; then
    return
  fi
  if [[ ! -t 0 ]]; then
    die "production PyPI upload needs an interactive confirmation or --yes"
  fi
  local expected="publish ${project_version}"
  local answer
  read -r -p "Upload ${project_name} ${project_version} to production PyPI? Type '${expected}' to continue: " answer
  if [[ "$answer" != "$expected" ]]; then
    die "production PyPI upload cancelled"
  fi
}

verify_install() {
  local repository="$1"
  local project_name="$2"
  local project_version="$3"
  local script_name="$4"
  local venv_dir
  venv_dir="$(mktemp -d "${TMPDIR:-/tmp}/${project_name}-${repository}-verify.XXXXXX")"
  log "Verifying install from ${repository} in ${venv_dir}"
  run "$PYTHON" -m venv "$venv_dir"
  local install_cmd=("$venv_dir/bin/python" -m pip install --no-cache-dir)
  case "$repository" in
    testpypi)
      install_cmd+=(
        --index-url https://test.pypi.org/simple/
        --extra-index-url https://pypi.org/simple
        "${project_name}==${project_version}"
      )
      ;;
    pypi)
      install_cmd+=("${project_name}==${project_version}")
      ;;
    *)
      die "unknown verify repository: $repository"
      ;;
  esac
  local attempts="${CODING_TOOLS_MCP_VERIFY_ATTEMPTS:-6}"
  local delay="${CODING_TOOLS_MCP_VERIFY_DELAY:-10}"
  local attempt
  for ((attempt = 1; attempt <= attempts; attempt++)); do
    if run "${install_cmd[@]}"; then
      break
    fi
    if ((attempt == attempts)); then
      die "could not install ${project_name}==${project_version} from ${repository}"
    fi
    log "Package index has not exposed ${project_version} yet; retrying in ${delay}s (${attempt}/${attempts})"
    sleep "$delay"
  done
  run "$venv_dir/bin/$script_name" --help >/dev/null
  run "$venv_dir/bin/python" -c "import coding_tools_mcp; assert coding_tools_mcp.__version__ == '${project_version}', coding_tools_mcp.__version__"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --testpypi)
      MODE="testpypi"
      ;;
    --pypi)
      MODE="pypi"
      ;;
    --both)
      MODE="both"
      ;;
    --skip-tests)
      RUN_TESTS=0
      ;;
    --skip-verify-install)
      VERIFY_INSTALL=0
      ;;
    --allow-dirty)
      ALLOW_DIRTY=1
      ;;
    -y|--yes)
      ASSUME_YES=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown argument: $1"
      ;;
  esac
  shift
done

ensure_module build build
ensure_module twine twine
check_git_clean

PROJECT_NAME="$(pyproject_value name)"
PROJECT_VERSION="$(pyproject_value version)"
SCRIPT_NAME="$(project_script_name)"
MODULE_VERSION="$("$PYTHON" -c 'import coding_tools_mcp; print(coding_tools_mcp.__version__)')"
if [[ "$MODULE_VERSION" != "$PROJECT_VERSION" ]]; then
  die "pyproject version ${PROJECT_VERSION} does not match coding_tools_mcp.__version__ ${MODULE_VERSION}"
fi

DIST_STEM="${PROJECT_NAME//-/_}-${PROJECT_VERSION}"
WHEEL="dist/${DIST_STEM}-py3-none-any.whl"
SDIST="dist/${DIST_STEM}.tar.gz"

log "Preparing ${PROJECT_NAME} ${PROJECT_VERSION}"
if [[ "$RUN_TESTS" == "1" ]]; then
  run make test
fi

run rm -rf dist
run "$PYTHON" -m build
[[ -f "$WHEEL" ]] || die "missing wheel: $WHEEL"
[[ -f "$SDIST" ]] || die "missing sdist: $SDIST"
run "$PYTHON" -m twine check "$WHEEL" "$SDIST"

if [[ "$MODE" == "testpypi" || "$MODE" == "both" ]]; then
  check_credentials testpypi
  run "$PYTHON" -m twine upload --non-interactive --repository testpypi "$WHEEL" "$SDIST"
  if [[ "$VERIFY_INSTALL" == "1" ]]; then
    verify_install testpypi "$PROJECT_NAME" "$PROJECT_VERSION" "$SCRIPT_NAME"
  fi
fi

if [[ "$MODE" == "pypi" || "$MODE" == "both" ]]; then
  check_credentials pypi
  confirm_production_upload "$PROJECT_NAME" "$PROJECT_VERSION"
  run "$PYTHON" -m twine upload --non-interactive "$WHEEL" "$SDIST"
  if [[ "$VERIFY_INSTALL" == "1" ]]; then
    verify_install pypi "$PROJECT_NAME" "$PROJECT_VERSION" "$SCRIPT_NAME"
  fi
fi

log "Release flow completed for ${PROJECT_NAME} ${PROJECT_VERSION} (${MODE})"
