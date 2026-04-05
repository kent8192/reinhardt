# Packer template for building reinhardt CI runner Golden AMIs.
# Supports both x64 (amd64) and arm64 (Graviton) architectures.

packer {
	required_plugins {
		amazon = {
			version = "~> 1.0.0"
			source  = "github.com/hashicorp/amazon"
		}
	}
}

# ---------------------------------------------------------------------------
# Variables
# ---------------------------------------------------------------------------

variable "runner_arch" {
	type        = string
	# Default to arm64 (AWS Graviton) for better price-performance ratio.
	# Graviton instances are ~20% cheaper than x86 equivalents with
	# comparable or better performance for CI workloads.
	default     = "arm64"
	description = "Runner architecture: x64 or arm64"

	validation {
		condition     = var.runner_arch == "x64" || var.runner_arch == "arm64"
		error_message = "Variable runner_arch must be either x64 or arm64."
	}
}

variable "aws_region" {
	type        = string
	default     = "us-east-1"
	description = "AWS region to build the AMI in"
}

variable "vpc_id" {
	type        = string
	default     = ""
	description = "VPC ID for the build instance (empty for default VPC)"
}

variable "subnet_id" {
	type        = string
	default     = ""
	description = "Subnet ID for the build instance (empty for default subnet)"
}

variable "ami_prefix" {
	type        = string
	default     = "reinhardt-ci-runner"
	description = "Prefix for the AMI name"
}

# ---------------------------------------------------------------------------
# Locals - architecture-dependent values
# ---------------------------------------------------------------------------

locals {
	source_ami_filter = var.runner_arch == "arm64" ? "ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-arm64-server-*" : "ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-amd64-server-*"
	build_instance_type = var.runner_arch == "arm64" ? "t4g.small" : "t3.medium"
	protoc_arch         = var.runner_arch == "arm64" ? "linux-aarch_64" : "linux-x86_64"
	awscli_arch         = var.runner_arch == "arm64" ? "aarch64" : "x86_64"
	cloudwatch_arch     = var.runner_arch == "arm64" ? "arm64" : "amd64"
}

# ---------------------------------------------------------------------------
# Source
# ---------------------------------------------------------------------------

source "amazon-ebs" "runner" {
	ami_name      = "${var.ami_prefix}-${var.runner_arch}-{{timestamp}}"
	instance_type = local.build_instance_type
	region        = var.aws_region
	vpc_id        = var.vpc_id
	subnet_id     = var.subnet_id
	ssh_username  = "ubuntu"

	# Ubuntu 22.04 AMIs from 2026-03 onwards require ed25519 keys for SSH.
	# RSA keys are rejected by the updated OpenSSH default configuration.
	temporary_key_pair_type = "ed25519"
	ssh_timeout             = "5m"

	source_ami_filter {
		filters = {
			name                = local.source_ami_filter
			root-device-type    = "ebs"
			virtualization-type = "hvm"
		}
		most_recent = true
		owners      = ["099720109477"] # Canonical
	}

	tags = {
		Name         = "${var.ami_prefix}-${var.runner_arch}"
		Architecture = var.runner_arch
		Project      = "reinhardt"
		ManagedBy    = "packer"
	}
}

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------

build {
	sources = ["source.amazon-ebs.runner"]

	# -- System packages -----------------------------------------------------
	provisioner "shell" {
		execute_command = "sudo sh -c '{{ .Vars }} {{ .Path }}'"
		inline = [
			"DEBIAN_FRONTEND=noninteractive apt-get update -qq",
			"DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends software-properties-common",
			"add-apt-repository -y universe",
			"apt-get update -qq",
			"DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \\",
			"  docker.io \\",
			"  docker-compose-v2 \\",
			"  build-essential \\",
			"  pkg-config \\",
			"  libssl-dev \\",
			"  mold \\",
			"  clang \\",
			"  lld \\",
			"  curl \\",
			"  jq \\",
			"  unzip \\",
			"  git \\",
			"  wget \\",
			"  ca-certificates",
		]
	}

	# -- GitHub CLI -----------------------------------------------------------
	provisioner "shell" {
		execute_command = "sudo sh -c '{{ .Vars }} {{ .Path }}'"
		inline = [
			"curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg",
			"chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg",
			"echo \"deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main\" | tee /etc/apt/sources.list.d/github-cli.list > /dev/null",
			"apt-get update -qq",
			"apt-get install -y gh",
		]
	}

	# -- protoc v28.3 (architecture-aware) ------------------------------------
	provisioner "shell" {
		execute_command = "sudo sh -c '{{ .Vars }} {{ .Path }}'"
		environment_vars = [
			"PROTOC_ARCH=${local.protoc_arch}",
		]
		inline = [
			"PROTOC_VERSION=28.3",
			"curl -fsSL -o /tmp/protoc.zip \"https://github.com/protocolbuffers/protobuf/releases/download/v$${PROTOC_VERSION}/protoc-$${PROTOC_VERSION}-$${PROTOC_ARCH}.zip\"",
			"unzip -o /tmp/protoc.zip -d /usr/local bin/protoc",
			"chmod +x /usr/local/bin/protoc",
			"rm -f /tmp/protoc.zip",
		]
	}

	# -- AWS CLI v2 (architecture-aware) --------------------------------------
	provisioner "shell" {
		execute_command = "sudo sh -c '{{ .Vars }} {{ .Path }}'"
		environment_vars = [
			"AWSCLI_ARCH=${local.awscli_arch}",
		]
		inline = [
			"curl -fsSL -o /tmp/awscliv2.zip \"https://awscli.amazonaws.com/awscli-exe-linux-$${AWSCLI_ARCH}.zip\"",
			"unzip -q /tmp/awscliv2.zip -d /tmp",
			"/tmp/aws/install",
			"rm -rf /tmp/aws /tmp/awscliv2.zip",
		]
	}

	# -- CloudWatch agent (architecture-aware) --------------------------------
	provisioner "shell" {
		execute_command = "sudo sh -c '{{ .Vars }} {{ .Path }}'"
		environment_vars = [
			"CW_ARCH=${local.cloudwatch_arch}",
		]
		inline = [
			"curl -fsSL -o /tmp/amazon-cloudwatch-agent.deb \"https://s3.amazonaws.com/amazoncloudwatch-agent/ubuntu/$${CW_ARCH}/latest/amazon-cloudwatch-agent.deb\"",
			"dpkg -i -E /tmp/amazon-cloudwatch-agent.deb",
			"rm -f /tmp/amazon-cloudwatch-agent.deb",
		]
	}

	# -- Docker configuration and image pre-pull ------------------------------
	provisioner "shell" {
		execute_command = "sudo sh -c '{{ .Vars }} {{ .Path }}'"
		inline = [
			"systemctl enable --now docker",
			"usermod -aG docker ubuntu",

			# Configure Docker for CI workloads
			"cat > /etc/docker/daemon.json << 'DAEMONJSON'",
			"{",
			"  \"storage-driver\": \"overlay2\",",
			"  \"max-concurrent-downloads\": 10",
			"}",
			"DAEMONJSON",
			"systemctl restart docker",

			# Pre-pull TestContainers images to eliminate pull latency during CI
			"docker pull postgres:17-alpine",
			"docker pull mysql:8.0",
			"docker pull redis:7-alpine",
			"docker pull rabbitmq:3-management-alpine",
		]
	}

	# -- TestContainers configuration -----------------------------------------
	provisioner "shell" {
		execute_command = "sudo sh -c '{{ .Vars }} {{ .Path }}'"
		inline = [
			"mkdir -p /home/ubuntu",
			"cat > /home/ubuntu/.testcontainers.properties << 'EOF'",
			"docker.client.strategy=org.testcontainers.dockerclient.UnixSocketClientProviderStrategy",
			"EOF",
			"chown -R ubuntu:ubuntu /home/ubuntu/.testcontainers.properties",
		]
	}

	# -- Manifest output ------------------------------------------------------
	post-processor "manifest" {
		output     = "manifest.json"
		strip_path = true
	}
}
