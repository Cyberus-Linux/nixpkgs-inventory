#!/usr/bin/env bash

shopt -s extglob
set -euo pipefail
PS4=" $ "

this_script="${BASH_SOURCE[0]}"
this_dir="$(readlink -f "${this_script%/*}")"
project_root="$(readlink -f "${this_dir}/../..")"

escape() {
	if (( $# > 0 )); then
		printf "%q" "$1"; shift
		if (( $# > 0 )); then
			printf " %q" "$@"
		fi
	fi
}

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

match_start() {
	local line="$1"; shift
	if [[ "${line}" =~ ^\<!--\ *\[COMMAND_HELP:(.*)\]\ --\>$ ]]; then
		printf "%s" "${BASH_REMATCH[1]}"
	fi
}

match_end() {
	local line="$1"; shift
	if [[ "${line}" =~ ^\<!--\ *\[/COMMAND_HELP:(.*)\]\ --\>$ ]]; then
		true
	else
		false
	fi
}

apply_template() {
	(
		local cmd=("$1"); shift
		printf '\n```\n'
		# Allow-list the allowed command templates.
		case "${cmd[0]}" in
			cake-unbaker|cu-fast-eval|cu-ingest-to-db)
				# Fall-through
				;;
			"cake-unbaker pkgs "+([a-z-]))
				# Unwrap two levels of subcommands
				cmd=("cake-unbaker" "pkgs" "${cmd#cake-unbaker pkgs *}")
				;;
			"cake-unbaker "+([a-z-]))
				# Unwrap one level of subcommands
				cmd=("cake-unbaker" "${cmd#cake-unbaker *}")
				;;
			*)
				abort "Command not allow-listed for updating the README: %s" "$(escape "${cmd[@]}")"
				;;
		esac
		# NOTE: stdin, stderr and stdout MUST not be the terminal, so that the `terminal_size` crate
		# ends-up giving `None`, and `clap` falls back to the environment.
		# 99 characters is the maximum GitLab will show before scrolling code blocks horizontally.
		# Test string: `123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789`
		# Let's not take chances, just in case.
		# Sadly, this means we can't have the error messages from `cargo` if this is being built.
		(COLUMNS=96 set -x; "${cmd[@]}" --help 2>/dev/null </dev/null | sed -e 's/\s\+$//')
		printf '```\n\n'
	)
}

update_templates() {
	local line=""
	local current_template=""
	(
		# Prevent whitespace stripping
		IFS=$'\n'
		while read -r "line"; do
			if [[ "${current_template}" == "" ]]; then
				current_template="$(match_start "$line")"
				# Pass-through the line
				printf "%s\n" "$line"
				# We just hit a template string?
				if [[ "$current_template" != "" ]]; then
					apply_template "$current_template"
				fi
			else
				# Do nothing except match the end
				if match_end "$line"; then
					# Pass-through the template
					printf "%s\n" "$line"
					current_template=""
				fi
			fi
		done
	)
}
 
update_templates < "$project_root/README.md" > "$project_root/README.md.tmp"
mv -v "$project_root/README.md.tmp" "$project_root/README.md"
