#!/bin/bash
# Test all examples

set -e

echo "🧪 Testing all examples..."
echo ""

# Determine if cargo-nextest is available
if command -v cargo-nextest &> /dev/null; then
	TEST_CMD="cargo nextest run"
	echo "📦 Using cargo-nextest for faster testing"
else
	TEST_CMD="cargo test"
	echo "📦 Using cargo test (install cargo-nextest for faster execution)"
fi
echo ""

# Test local examples
echo "=== Testing Local Examples ==="
echo ""

cd local || {
	echo "❌ Error: local/ directory not found"
	exit 1
}

FAILED_EXAMPLES=()

for example in examples-*/; do
	if [ ! -d "$example" ]; then
		continue
	fi

	example_name="${example%/}"
	echo "📦 Testing ${example_name}..."

	cd "$example" || {
		echo "❌ Error: Cannot enter ${example_name} directory"
		FAILED_EXAMPLES+=("$example_name")
		cd ..
		continue
	}

	# Run tests
	if $TEST_CMD --all-features; then
		echo "✅ ${example_name} tests passed"
	else
		echo "❌ ${example_name} tests failed"
		FAILED_EXAMPLES+=("$example_name")
	fi

	cd .. || exit 1
	echo ""
done

cd .. || exit 1

# Test remote common crates
echo "=== Testing Remote Common Crates ==="
echo ""

cd remote || {
	echo "❌ Error: remote/ directory not found"
	exit 1
}

echo "📦 Testing remote workspace..."
if cargo test --workspace --all-features; then
	echo "✅ Remote common crates tests passed"
else
	echo "❌ Remote common crates tests failed"
	FAILED_EXAMPLES+=("remote/common")
fi

cd .. || exit 1
echo ""

# Summary
echo "=== Test Summary ==="
echo ""

if [ ${#FAILED_EXAMPLES[@]} -eq 0 ]; then
	echo "✅ All tests passed!"
	exit 0
else
	echo "❌ Failed examples:"
	for failed in "${FAILED_EXAMPLES[@]}"; do
		echo "   - $failed"
	done
	echo ""
	echo "Total failed: ${#FAILED_EXAMPLES[@]}"
	exit 1
fi
