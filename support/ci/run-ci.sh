#!/usr/bin/env bash

#
# This script is the entrypoint to run the checks on CI.
#
# > [!TIP]
# > It's a good idea to run this script *before* contributing back your changes!
#

set -euo pipefail

this_script="${BASH_SOURCE[0]}"
this_dir="$(readlink -f "${this_script%/*}")"
project_root="$(readlink -f "${this_dir}/../..")"

#
# Aliases / helpers
#

command_nix_build="$(type -fp nix-build)"
nix-build() {
	(set -x
	"$command_nix_build" "$@"
	)
}

command_nix_instantiate="$(type -fp nix-instantiate)"
nix-instantiate() {
	(set -x
	"$command_nix_instantiate" "$@"
	)
}

eprintln() {
	pattern="$1"; shift
	# shellcheck disable=2059
	printf "${pattern}\n" "$@" >&2
}

msg() {
	printf "\n :: %s\n" "$@" >&2
}

#
# Checks
#

check-eval() {
	time nix-instantiate "${project_root}/support/nix/checks.nix"
}

# NOTE: using `--keep-going` so we build the most we can. This may be a
#       mistake, as it might make finding the failures harder than necessary.

check-simple-builds() {
	time nix-build \
		--no-out-link \
		--keep-going \
		"${project_root}/support/nix/checks.nix"
}

check-expensive-builds() {
	time nix-build \
		--keep-going \
		--no-out-link \
		--max-jobs 1 \
		--arg with-expensive-tests true \
		"${project_root}/support/nix/checks.nix"
}

#
# Arguments parsing
#

with_expensive=0
usage() {
	cat <<-EOF
	${this_script}: [options]
	
		--help                    prints this help
		--enable-expensive        enables expensive checks
	EOF
}

while (( $# > 0 )); do
	arg="$1"; shift
	case "$arg" in
		--enable-expensive)
			with_expensive=1
			;;
		-h|--help)
			usage
			exit 0
			;;
		*)
			eprintln "Error: Unexpected argument %s" "$arg"
			usage >&2
			exit 1
			;;
	esac
done

#
# The script
#

msg "Checking that Nix-based checks evaluates."
if ! check-eval; then
	eprintln "Failed to produce derivations for CI."
	eprintln ""
	eprintln " → Are the Nix expressions valid?"
	exit 1
fi

msg "Building simpler Nix-based checks."
if ! check-simple-builds; then
	eprintln "Failed to build simple checks."
	exit 1
fi

if (( with_expensive )); then
	msg "Building expensive checks."
	if ! check-expensive-builds; then
		eprintln "Failed to build simple checks."
		exit 1
	fi
else
	msg "Skipping expensive checks."
fi
