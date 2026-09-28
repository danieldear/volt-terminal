# Source explicitly at the end of .bashrc. Supports macOS Bash 3.2 as well.
[[ $- == *i* && ${TERM_PROGRAM-} == volt && -z ${_VOLT_BASH_INTEGRATION-} ]] || return 0
_VOLT_BASH_INTEGRATION=1

_volt_precmd() {
    local last_status=$?
    if [[ -t 1 ]]; then
        builtin printf '\033]133;D;%d\007\033]133;A\007' "$last_status"
    fi
    # Existing PROMPT_COMMAND sees the original command's status.
    return "$last_status"
}
# Bash 5.1+ executes array entries separately. Older Bash executes only index 0;
# keep that behavior, rather than silently hiding the existing index-0 hook.
if (( BASH_VERSINFO[0] > 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] >= 1) )) &&
    [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == 'declare -a '* ]]; then
    PROMPT_COMMAND=(_volt_precmd "${PROMPT_COMMAND[@]}")
else
    PROMPT_COMMAND="_volt_precmd${PROMPT_COMMAND:+; $PROMPT_COMMAND}"
fi
# PS0 is supported in Bash 4.4+. Do not install/replace a DEBUG trap. Bash 3.2
# still gets prompt navigation and exit status, but not command start/duration.
if (( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 4) )); then
    PS0=$'\033]133;C\007'${PS0-}
fi
