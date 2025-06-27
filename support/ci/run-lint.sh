#!/usr/bin/env nix-shell
#! nix-shell ../../shell.nix
#! nix-shell -i bash

set -euo pipefail
PS4=" $ "

this_script="${BASH_SOURCE[0]}"
this_dir="$(readlink -f "${this_script%/*}")"
project_root="$(readlink -f "${this_dir}/../..")"

#
# Aliases / helpers
#

eprintln() {
	pattern="$1"; shift
	# shellcheck disable=2059
	printf "${pattern}\n" "$@" >&2
}

abort() {
	format="$1"; shift
	eprintln "\n  ❌ $format\n" "$@"
	return 1
}

msg() {
	printf "\n :: %s\n" "$@" >&2
}

unstaged_changes() {
	# The grep checks the second character of each lines.
	# A file that is staged and has no unstaged changes will have a space at the second char.
	git status --porcelain=v1 --untracked-files=no | grep '^.[^ ]'
}

repo_is_clean() {
	[[ "$(unstaged_changes)" == "" ]]
}

abort_if_unclean() {
	msg="$1"; shift
	if ! repo_is_clean; then
		(
			# Re-use the abort message format.
			abort "$msg" || :
			unstaged_changes
		) >&2
		exit 1
	fi
}

abort_if_unclean_show_diff() {
	msg="$1"; shift
	if ! (abort_if_unclean "$msg"); then
		(
			printf "\n"
			# Using `cat` to force a pipeline so `git` does not use the pager.
			(set -x; git diff) | cat
		) >&2
		exit 1
	fi
}

#
# Checks
#

cd "$project_root"

msg "Checking if the repository is clean..."
abort_if_unclean "The repository has unstaged changes."

msg "Checking if generated documentation is up-to-date..."

(set -x; support/documentation/update-readme.sh)

abort_if_unclean_show_diff "The README was not up-to-date."

msg "Checking rust code formatting and linting"

# It is safe to use `--fix --allow-dirty` here since this would not unstage
# changes already staged, and we already checked for uncommitted changes.
if ! (set -x; cargo clippy --all --fix --allow-dirty -- --deny warnings); then
	abort "Clippy has issues with the code, but no suggestions to apply..."
fi

abort_if_unclean_show_diff "Clippy has suggestions to be committed."

(set -x; cargo fmt)

abort_if_unclean_show_diff "The rust code differed after formatting."
