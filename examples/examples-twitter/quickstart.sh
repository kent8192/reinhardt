#!/usr/bin/env bash
set -euo pipefail

# Reinhardt Twitter Demo - Quick Start
# Run this script from the examples/examples-twitter directory

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info()  { echo -e "${BLUE}[INFO]${NC} $1"; }
ok()    { echo -e "${GREEN}[OK]${NC} $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

echo ""
echo "=========================================="
echo "  Reinhardt Twitter Demo - Quick Start"
echo "=========================================="
echo ""

# -----------------------------------------------
# 1. Check prerequisites
# -----------------------------------------------
info "Checking prerequisites..."

MISSING=()

if ! command -v rustc &> /dev/null; then
    MISSING+=("rustc (https://rustup.rs)")
fi

if ! command -v cargo &> /dev/null; then
    MISSING+=("cargo (https://rustup.rs)")
fi

if ! command -v docker &> /dev/null; then
    MISSING+=("docker (https://docs.docker.com/get-docker/)")
fi

if ! command -v wasm-pack &> /dev/null; then
    MISSING+=("wasm-pack (https://rustwasm.github.io/wasm-pack/installer/)")
fi

if ! command -v cargo-make &> /dev/null; then
    if ! cargo make --version &> /dev/null 2>&1; then
        MISSING+=("cargo-make (cargo install cargo-make)")
    fi
fi

# Check wasm32 target
if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
    MISSING+=("wasm32-unknown-unknown target (rustup target add wasm32-unknown-unknown)")
fi

if [ ${#MISSING[@]} -ne 0 ]; then
    error "Missing prerequisites:"
    for item in "${MISSING[@]}"; do
        echo "  - $item"
    done
    echo ""
    echo "Install all prerequisites and run this script again."
    exit 1
fi

ok "All prerequisites found"

# -----------------------------------------------
# 2. Check Docker is running
# -----------------------------------------------
info "Checking Docker daemon..."

if ! docker info &> /dev/null; then
    error "Docker daemon is not running. Please start Docker Desktop and try again."
    exit 1
fi

ok "Docker is running"

# -----------------------------------------------
# 3. Setup local settings
# -----------------------------------------------
info "Setting up local configuration..."

if [ ! -f settings/local.toml ]; then
    cp settings/local.example.toml settings/local.toml
    # Update DB name to match docker-compose.yml
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' 's/examples-twitter_dev/examples-twitter_db/' settings/local.toml
    else
        sed -i 's/examples-twitter_dev/examples-twitter_db/' settings/local.toml
    fi
    ok "Created settings/local.toml from template"
else
    ok "settings/local.toml already exists"
fi

# -----------------------------------------------
# 4. Start PostgreSQL via Docker Compose
# -----------------------------------------------
info "Starting PostgreSQL..."

docker compose up -d

info "Waiting for PostgreSQL to be ready..."
RETRIES=30
until docker compose exec -T postgres pg_isready -U postgres &> /dev/null; do
    RETRIES=$((RETRIES - 1))
    if [ $RETRIES -le 0 ]; then
        error "PostgreSQL failed to start within timeout"
        exit 1
    fi
    sleep 1
done

ok "PostgreSQL is ready"

# -----------------------------------------------
# 5. Run database migrations
# -----------------------------------------------
info "Running database migrations..."

cargo run --bin examples-twitter migrate

ok "Migrations complete"

# -----------------------------------------------
# 6. Build WASM and start server
# -----------------------------------------------
info "Building WASM frontend and starting server..."
info "This may take a few minutes on the first run..."

echo ""
echo "=========================================="
echo "  Starting Reinhardt Twitter Demo"
echo "=========================================="
echo ""
echo "  Frontend:   http://127.0.0.1:8000"
echo "  Admin:      http://127.0.0.1:8000/admin/"
echo "  Swagger UI: http://127.0.0.1:8000/api/docs/"
echo "  Redoc:      http://127.0.0.1:8000/api/redoc/"
echo ""
echo "  Press Ctrl+C to stop the server"
echo "=========================================="
echo ""

cargo make dev
