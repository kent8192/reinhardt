'use strict';

const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const { readFileSync } = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');

const root = execFileSync('git', ['rev-parse', '--show-toplevel'], {
	encoding: 'utf8',
}).trim();
const workflow = readFileSync(
	path.join(root, '.github/workflows/warn-invalid-breaking-change.yml'),
	'utf8',
);
const script = workflow.split('          script: |\n')[1];
assert.ok(script, 'the test must execute the actual workflow policy');
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
const policy = new AsyncFunction('github', 'context', 'core', script);
const marker = '<!-- invalid-breaking-change-target-warning -->';

function pullRequest(overrides = {}) {
	return {
		number: 6276,
		title: 'fix(pages): support RadioInput',
		labels: [],
		base: { ref: 'main' },
		head: { ref: 'fix/issue-6270-form-radio-input' },
		...overrides,
	};
}

async function run(pr, options = {}) {
	const result = { failures: [], comments: [], listed: 0, reads: [] };
	const context = {
		repo: { owner: 'kent8192', repo: 'reinhardt-web' },
		payload: { pull_request: options.payload ?? pr },
	};
	const github = {
		rest: {
			pulls: {
				get: async (request) => {
					result.reads.push(request);
					return { data: pr };
				},
			},
			issues: {
				listComments: Symbol('listComments'),
				createComment: async (comment) => {
					assert.equal(result.failures.length, 1, 'fail before posting');
					if (options.postError) throw new Error('HTTP 403');
					result.comments.push(comment);
				},
			},
		},
		paginate: async (endpoint, request) => {
			assert.equal(endpoint, github.rest.issues.listComments);
			assert.deepEqual(request, {
				owner: 'kent8192', repo: 'reinhardt-web',
				issue_number: 6276, per_page: 100,
			});
			assert.equal(result.failures.length, 1, 'fail before deduplication');
			result.listed += 1;
			return options.comments ?? [];
		},
	};
	const core = {
		setFailed: (message) => result.failures.push(message),
		info: () => {},
	};
	try {
		await policy(github, context, core);
	} catch (error) {
		result.error = error.message;
	}
	assert.deepEqual(result.reads, [{
		owner: 'kent8192', repo: 'reinhardt-web', pull_number: 6276,
	}]);
	return result;
}

for (const title of [
	'fix!: support RadioInput',
	'fix!(pages): support RadioInput',
	'fix(pages)!: support RadioInput',
]) {
	test(`rejects a main-targeting breaking title: ${title}`, async () => {
		const result = await run(pullRequest({ title }));
		assert.equal(result.error, undefined);
		assert.equal(result.failures.length, 1);
		assert.equal(result.comments.length, 1);
		assert.equal(result.comments[0].issue_number, 6276);
		assert.ok(result.comments[0].body.startsWith(marker));
	});
}

test('a breaking-change label fails even without a breaking title', async () => {
	const result = await run(pullRequest({ labels: [{ name: 'breaking-change' }] }));
	assert.equal(result.failures.length, 1);
	assert.equal(result.comments.length, 1);
});

test('an existing warning still fails without posting a duplicate', async () => {
	const result = await run(pullRequest({ title: 'fix!(pages): support RadioInput' }), {
		comments: [{ body: 'unrelated' }, { body: `${marker}\nExisting warning` }],
	});
	assert.equal(result.error, undefined);
	assert.equal(result.failures.length, 1);
	assert.equal(result.listed, 1);
	assert.deepEqual(result.comments, []);
});

test('comment delivery failure cannot turn an invalid target green', async () => {
	const result = await run(pullRequest({ title: 'fix!: support RadioInput' }), {
		postError: true,
	});
	assert.equal(result.error, 'HTTP 403');
	assert.equal(result.failures.length, 1);
});

for (const pr of [
	pullRequest(),
	pullRequest({ title: 'fix!: support RadioInput', base: { ref: 'develop/0.4.0' } }),
	pullRequest({ title: 'feat!: release transition', head: { ref: 'develop/0.4.0' } }),
]) {
	test(`accepts ${pr.head.ref} -> ${pr.base.ref}: ${pr.title}`, async () => {
		const result = await run(pr);
		assert.equal(result.error, undefined);
		assert.deepEqual(result.failures, []);
		assert.deepEqual(result.comments, []);
		assert.equal(result.listed, 0);
	});
}

test('an unversioned develop branch does not bypass the check', async () => {
	const result = await run(pullRequest({
		title: 'feat!: change API', head: { ref: 'develop/next' },
	}));
	assert.equal(result.failures.length, 1);
});

test('current breaking metadata overrides a stale nonbreaking event', async () => {
	const result = await run(pullRequest({ title: 'fix!: support RadioInput' }), {
		payload: pullRequest(),
	});
	assert.equal(result.failures.length, 1);
});

test('removing a mistaken label clears failure despite an old warning', async () => {
	const result = await run(pullRequest(), {
		payload: pullRequest({ labels: [{ name: 'breaking-change' }] }),
		comments: [{ body: marker }],
	});
	assert.deepEqual(result.failures, []);
	assert.equal(result.listed, 0);
});

test('retargeted current metadata overrides an old main event', async () => {
	const result = await run(pullRequest({
		title: 'fix!: support RadioInput', base: { ref: 'develop/0.4.0' },
	}), { payload: pullRequest({ title: 'fix!: support RadioInput' }) });
	assert.deepEqual(result.failures, []);
});

test('metadata changes trigger the trusted required check', () => {
	assert.match(workflow, /pull_request_target:/);
	const events = workflow.match(/types: \[([^\]]+)\]/)[1].split(',').map(s => s.trim());
	for (const event of ['edited', 'labeled', 'unlabeled', 'synchronize', 'ready_for_review']) {
		assert.ok(events.includes(event), `missing ${event} trigger`);
	}
	assert.doesNotMatch(workflow, /actions\/checkout/);
});
