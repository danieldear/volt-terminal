# Source explicitly at the end of .zshrc. No startup files are edited by Volt.
[[ -o interactive && $TERM_PROGRAM == volt && -z ${_VOLT_ZSH_INTEGRATION-} ]] || return 0
_VOLT_ZSH_INTEGRATION=1

_volt_precmd() {
    local last_status=$?
    # Do not write protocol bytes into redirected output or prompt redraws.
    if [[ -t 1 ]] && ! builtin zle; then
        builtin printf '\033]133;D;%d\007\033]133;A\007' "$last_status"
    fi
    return 0
}
_volt_preexec() {
    [[ -t 1 ]] && builtin printf '\033]133;C\007'
    return 0
}
autoload -Uz add-zsh-hook
add-zsh-hook precmd _volt_precmd
add-zsh-hook preexec _volt_preexec
