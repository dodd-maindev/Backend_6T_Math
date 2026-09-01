#!/bin/bash
set -e

echo "=== Starting 6T MATH Backend Native Deployment ==="

TARGET_DIR="/var/www/backend-6t-math"
mkdir -p "$TARGET_DIR"
cd "$TARGET_DIR"

# 1. Sync git repository
if [ ! -d ".git" ]; then
  echo "Initializing Git in existing directory..."
  git init
  git remote add origin https://github.com/dodd-maindev/Backend_6T_Math.git || git remote set-url origin https://github.com/dodd-maindev/Backend_6T_Math.git
fi

echo "Fetching and resetting to latest origin/main..."
git fetch origin main
git reset --hard origin/main

# 2. Ensure C Compiler & Linker (cc/gcc) and Rust toolchain exist
if ! command -v cc &> /dev/null; then
  echo "Installing C build tools (build-essential, pkg-config, libssl-dev)..."
  apt-get update -qq && apt-get install -y -qq build-essential pkg-config libssl-dev
fi

source "$HOME/.cargo/env" || export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v cargo &> /dev/null; then
  echo "Installing Rust toolchain..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi

# 3. Build native release binary
echo "Building Rust release binary..."
cargo build --release

# 4. Ensure Systemd Service exists
SERVICE_FILE="/etc/systemd/system/backend-6t-math.service"
if [ ! -f "$SERVICE_FILE" ]; then
  echo "Creating Systemd service..."
  cat << 'EOF' > "$SERVICE_FILE"
[Unit]
Description=6T Math Backend Rust Service
After=network.target postgresql.service

[Service]
Type=simple
User=root
WorkingDirectory=/var/www/backend-6t-math
ExecStart=/var/www/backend-6t-math/target/release/backend-6t-math
Restart=always
RestartSec=5
EnvironmentFile=/var/www/backend-6t-math/.env

[Install]
WantedBy=multi-user.target
EOF
  systemctl daemon-reload
  systemctl enable backend-6t-math
fi

# 5. Restart service
echo "Restarting backend-6t-math service..."
systemctl daemon-reload
systemctl restart backend-6t-math
systemctl status backend-6t-math --no-pager

echo "=== Deployment Completed Successfully! ==="
