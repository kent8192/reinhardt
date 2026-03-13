# OIDC identity provider for GitHub Actions.
# Allows GitHub Actions workflows to assume IAM roles without long-lived credentials.
resource "aws_iam_openid_connect_provider" "github_actions" {
  url = "https://token.actions.githubusercontent.com"

  client_id_list  = ["sts.amazonaws.com"]
  thumbprint_list = ["ffffffffffffffffffffffffffffffffffffffff"]
}

# IAM role for the AMI builder GitHub Actions workflow (build-runner-ami.yml).
# Assumed via OIDC federation from GitHub Actions.
resource "aws_iam_role" "github_actions_ami_builder" {
  name = "${var.prefix}-gha-ami-builder"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Principal = {
          Federated = aws_iam_openid_connect_provider.github_actions.arn
        }
        Action = "sts:AssumeRoleWithWebIdentity"
        Condition = {
          StringEquals = {
            "token.actions.githubusercontent.com:aud" = "sts.amazonaws.com"
            "token.actions.githubusercontent.com:sub" = "repo:${var.github_owner}/${var.github_repository}:ref:refs/heads/main"
          }
        }
      }
    ]
  })
}

# EC2 permissions for Packer AMI builds.
resource "aws_iam_role_policy" "ami_builder_ec2" {
  name = "packer-ec2-ami-build"
  role = aws_iam_role.github_actions_ami_builder.name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid    = "PackerBuild"
        Effect = "Allow"
        Action = [
          "ec2:AttachVolume",
          "ec2:AuthorizeSecurityGroupIngress",
          "ec2:CopyImage",
          "ec2:CreateImage",
          "ec2:CreateKeypair",
          "ec2:CreateSecurityGroup",
          "ec2:CreateSnapshot",
          "ec2:CreateTags",
          "ec2:CreateVolume",
          "ec2:DeleteKeyPair",
          "ec2:DeleteSecurityGroup",
          "ec2:DeleteSnapshot",
          "ec2:DeleteVolume",
          "ec2:DeregisterImage",
          "ec2:DescribeImageAttribute",
          "ec2:DescribeImages",
          "ec2:DescribeInstances",
          "ec2:DescribeInstanceStatus",
          "ec2:DescribeRegions",
          "ec2:DescribeSecurityGroups",
          "ec2:DescribeSnapshots",
          "ec2:DescribeSubnets",
          "ec2:DescribeVolumes",
          "ec2:DescribeVpcs",
          "ec2:DetachVolume",
          "ec2:GetPasswordData",
          "ec2:ModifyImageAttribute",
          "ec2:ModifyInstanceAttribute",
          "ec2:ModifySnapshotAttribute",
          "ec2:RegisterImage",
          "ec2:RunInstances",
          "ec2:StopInstances",
          "ec2:TerminateInstances",
        ]
        Resource = "*"
      }
    ]
  })
}

# SSM permissions for updating the Golden AMI parameter after build.
resource "aws_iam_role_policy" "ami_builder_ssm" {
  name = "ssm-put-parameter-ami"
  role = aws_iam_role.github_actions_ami_builder.name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid    = "UpdateAmiParameter"
        Effect = "Allow"
        Action = ["ssm:PutParameter"]
        Resource = [
          "arn:aws:ssm:${var.aws_region}:${var.aws_account_id}:parameter/${var.prefix}/runner-ami-id",
        ]
      }
    ]
  })
}

# Store the IAM role ARN as a GitHub Actions secret for the build-runner-ami workflow.
resource "github_actions_secret" "aws_role_arn" {
  repository      = var.github_repository
  secret_name     = "AWS_ROLE_ARN"
  plaintext_value = aws_iam_role.github_actions_ami_builder.arn
}
