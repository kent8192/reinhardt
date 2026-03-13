terraform {
  required_version = ">= 1.10"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    github = {
      source  = "integrations/github"
      version = "~> 6.0"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.0"
    }
  }

  # Partial backend: supply values via -backend-config=backend.tfvars
  # See backend.tfvars.example for required values
  backend "s3" {
    use_lockfile = true # Terraform >= 1.10 native locking (no DynamoDB needed)
  }
}

provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project   = "reinhardt"
      ManagedBy = "terraform"
      Component = "github-runners"
    }
  }
}

provider "github" {
  owner = var.github_owner
  app_auth {
    id              = var.github_app_id
    installation_id = var.github_app_installation_id
    pem_file        = base64decode(var.github_app_key_base64)
  }
}

# Webhook secret auto-generation
resource "random_password" "webhook_secret" {
  length  = 32
  special = false
}

# Default VPC data sources (use default VPC for simplicity)
data "aws_vpc" "default" {
  default = true
}

data "aws_availability_zones" "available" {
  state = "available"

  filter {
    name   = "opt-in-status"
    values = ["opt-in-not-required"]
  }

  # Exclude us-east-1e (zone ID use1-az3) because it doesn't support required Graviton instance types
  exclude_zone_ids = ["use1-az3"]
}

data "aws_subnets" "default" {
  filter {
    name   = "vpc-id"
    values = [data.aws_vpc.default.id]
  }
  filter {
    name   = "availability-zone"
    values = data.aws_availability_zones.available.names
  }
}
