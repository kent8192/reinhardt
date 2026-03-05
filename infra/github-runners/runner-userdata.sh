#!/bin/bash
# Pre-installation setup for reinhardt CI runners.
# Executed by github-aws-runners module before runner registration.

set -eo pipefail

# Install required system packages
apt-get update -qq
apt-get install -y --no-install-recommends \
	docker.io \
	docker-compose-v2 \
	build-essential \
	pkg-config \
	libssl-dev \
	mold \
	clang \
	lld \
	curl \
	jq \
	unzip

# Install GitHub CLI (required by CI workflows that use 'gh' commands)
curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg
chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | tee /etc/apt/sources.list.d/github-cli.list > /dev/null
apt-get update -qq
apt-get install -y gh

# Install protoc v28 (Ubuntu 22.04 ships v3.12 which lacks proto3 optional support)
PROTOC_VERSION=28.3
curl -fsSL -o /tmp/protoc.zip "https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-linux-x86_64.zip"
unzip -o /tmp/protoc.zip -d /usr/local bin/protoc
chmod +x /usr/local/bin/protoc
rm -f /tmp/protoc.zip

# Enable Docker (required for TestContainers)
systemctl enable --now docker
usermod -aG docker ubuntu

# Optimize Docker for CI workloads
cat > /etc/docker/daemon.json << 'EOF'
{
  "max-concurrent-downloads": 10,
  "storage-driver": "overlay2"
}
EOF
systemctl restart docker

# Pre-pull TestContainers images (eliminates pull latency during CI jobs)
docker pull postgres:17-alpine
docker pull mysql:8.0
docker pull redis:7-alpine
docker pull rabbitmq:3-management-alpine

# Create .testcontainers.properties to force Docker socket (not Podman)
mkdir -p /home/ubuntu
cat > /home/ubuntu/.testcontainers.properties << 'EOF'
docker.client.strategy=org.testcontainers.dockerclient.UnixSocketClientProviderStrategy
EOF
chown -R ubuntu:ubuntu /home/ubuntu/.testcontainers.properties
