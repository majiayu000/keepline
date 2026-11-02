#!/bin/sh
# CI test step: the whole workspace once; then each failed case of managed_session, and only
# those, once more on its own.
#
# managed_session drives real terminals, agents and the host against wall-clock limits. On a
# shared 3-core runner one random case in it fails per run (a different one each time), the
# same way it does locally under load. A failure anywhere else, a compile error, or a
# managed_session case that fails again alone still fails the step. Every retried case is
# listed so it can be made deterministic later.
set -u
log=$(mktemp)
# A pipeline's status is tee's; keep cargo's own.
{
	cargo test --workspace --no-fail-fast 2>&1
	echo $? >"$log.status"
} | tee "$log"
if [ "$(cat "$log.status")" = 0 ]; then
	exit 0
fi
if grep -q '^error: could not compile' "$log"; then
	echo "ci-test: compile error, no retry"
	exit 1
fi
# "Running tests/managed_session.rs (...)" starts that binary's output; doc-tests and unit
# tests have their own headers.
failures=$(awk '
	/^ *Running / { bin = $2 }
	/^ *Doc-tests / { bin = "doc-tests" }
	/^test .* \.\.\. FAILED$/ { print bin "\t" $2 }
' "$log")
if [ -z "$failures" ]; then
	echo "ci-test: cargo test failed without a failed case to retry"
	exit 1
fi
others=$(printf '%s\n' "$failures" | grep -v '^tests/managed_session\.rs	' || true)
if [ -n "$others" ]; then
	echo "ci-test: failures outside managed_session, no retry:"
	printf '%s\n' "$others"
	exit 1
fi
status=0
for case in $(printf '%s\n' "$failures" | cut -f2); do
	echo "ci-test: retrying managed_session::$case"
	if cargo test -p keeplined --test managed_session -- --exact "$case"; then
		echo "::warning::managed_session::$case failed once and passed on retry"
	else
		echo "::error::managed_session::$case failed twice"
		status=1
	fi
done
exit $status
