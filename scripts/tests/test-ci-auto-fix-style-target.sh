#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"
POLICY_FILE="$ROOT_DIR/.github/scripts/auto-fix-style-target.cjs"
WORKFLOW_FILE="$ROOT_DIR/.github/workflows/ci.yml"

if [[ ! -f "$POLICY_FILE" ]]; then
	echo "FAIL: expected shared auto-fix target policy at $POLICY_FILE" >&2
	exit 1
fi

node - "$POLICY_FILE" "$WORKFLOW_FILE" <<'NODE'
const assert = require('node:assert/strict');
const fs = require('node:fs');

const policyFile = process.argv[2];
const workflowFile = process.argv[3];
const { classifyBranchProtection, runAutoFixTargetPolicy } = require(policyFile);

async function main() {

assert.deepEqual(
  classifyBranchProtection({ protected: false }, []),
  { eligible: true, reason: 'eligible' },
);

assert.deepEqual(
  classifyBranchProtection(
    { protected: true, protection: { enabled: false } },
    [{ type: 'non_fast_forward' }],
  ),
  { eligible: true, reason: 'eligible-non-fast-forward-only' },
);

assert.deepEqual(
  classifyBranchProtection(
    { protected: true, protection: { enabled: false } },
    [{ type: 'required_pull_request' }],
  ),
  { eligible: false, reason: 'protected-branch' },
);

assert.deepEqual(
  classifyBranchProtection(
    { protected: true, protection: { enabled: false } },
    [],
  ),
  { eligible: false, reason: 'protected-branch' },
);

assert.deepEqual(
  classifyBranchProtection(
    { protected: true, protection: { enabled: true } },
    [{ type: 'non_fast_forward' }],
  ),
  { eligible: false, reason: 'protected-branch' },
);

const workflow = fs.readFileSync(workflowFile, 'utf8');
const sharedPolicyCalls = workflow.match(/runAutoFixTargetPolicy\(/g) ?? [];
assert.equal(
  sharedPolicyCalls.length,
  2,
  'initial and pre-commit target checks must share the same policy',
);

const trustedBaseCheckouts = workflow.match(
  /ref: \$\{\{ github\.event\.pull_request\.base\.sha \}\}/g,
) ?? [];
assert.equal(
  trustedBaseCheckouts.length,
  2,
  'both target checks must load policy code from the trusted base commit',
);

const trustedPolicyLoads = workflow.match(
  /require\('\.\/\.auto-fix-policy\/\.github\/scripts\/auto-fix-style-target\.cjs'\)/g,
) ?? [];
assert.equal(
  trustedPolicyLoads.length,
  2,
  'both target checks must execute the trusted base policy copy',
);

assert.match(
  workflow,
  /git add -A -- ':!\.auto-fix-policy\/\*\*'/,
  'patch export must exclude the trusted policy checkout path',
);

assert.match(
  workflow,
  /git apply --index --whitespace=nowarn --exclude='\.auto-fix-policy\/\*\*'/,
  'patch apply must exclude the trusted policy checkout path',
);

assert.match(
  workflow,
  /git diff --cached --name-only -- \.auto-fix-policy \| grep -q \./,
  'write job must fail closed if trusted policy checkout files are staged',
);

assert.match(
  workflow,
  /git cat-file blob ":\$path" \\\n+\s+\| base64 -w0 \\\n+\s+\| jq -ncR --arg p "\$path" '\{path: \$p, contents: input\}'/,
  'GraphQL additions must stream staged blobs without process arguments',
);

assert.doesNotMatch(
  workflow,
  /--arg c "\$contents"/,
  'GraphQL additions must not carry file contents in a process argument',
);

const outputs = new Map();
const paginateCalls = [];
await runAutoFixTargetPolicy({
  github: {
    rest: {
      repos: {
        getBranch: async () => ({
          data: { protected: true, protection: { enabled: false } },
        }),
      },
    },
    paginate: async (route, parameters) => {
      paginateCalls.push({ route, parameters });
      return [
        { type: 'non_fast_forward' },
        { type: 'required_pull_request' },
      ];
    },
  },
  context: {
    payload: {
      pull_request: {
        head: {
          ref: 'feature/test',
          repo: { full_name: 'kent8192/reinhardt-web' },
        },
      },
    },
    repo: { owner: 'kent8192', repo: 'reinhardt-web' },
  },
  core: {
    notice: () => {},
    setOutput: (name, value) => outputs.set(name, value),
    warning: () => {},
  },
  noticePrefix: 'test',
});

assert.deepEqual(paginateCalls, [
  {
    route: 'GET /repos/{owner}/{repo}/rules/branches/{branch}',
    parameters: {
      owner: 'kent8192',
      repo: 'reinhardt-web',
      branch: 'feature/test',
    },
  },
]);
assert.equal(outputs.get('eligible'), 'false');
assert.equal(outputs.get('reason'), 'protected-branch');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
NODE

echo "PASS: CI style auto-fix target policy"
