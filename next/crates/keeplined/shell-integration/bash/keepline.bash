# Keepline bash integration. Keepline starts interactive bash with --init-file pointing
# here, which bash reads instead of ~/.bashrc; so this file first loads the user's own
# startup files the way bash itself would, then reports the working directory with
# OSC 7 (file://host/path, percent-encoded) so new tabs and splits open where you are, and
# marks where each command starts and ends (OSC 133 C, and D with its exit status) so
# plugins can hear about finished commands.
if [[ -n "${KEEPLINE_BASH_LOGIN-}" ]]; then
    builtin unset KEEPLINE_BASH_LOGIN
    [[ -r /etc/profile ]] && builtin source /etc/profile
    for _keepline_file in ~/.bash_profile ~/.bash_login ~/.profile; do
        if [[ -r "$_keepline_file" ]]; then
            builtin source "$_keepline_file"
            break
        fi
    done
else
    [[ -r ~/.bashrc ]] && builtin source ~/.bashrc
fi
builtin unset _keepline_file KEEPLINE_SHELL_INTEGRATION_DIR

_keepline_report_pwd() {
    # Byte by byte, so a path in any language encodes as its UTF-8 bytes.
    builtin local LC_ALL=C dir="$PWD" out='' c i n
    for (( i = 0; i < ${#dir}; i++ )); do
        c="${dir:i:1}"
        case "$c" in
            [-/._~A-Za-z0-9]) out+="$c" ;;
            *)
                # bash 3.2 reads bytes past 127 as negative.
                builtin printf -v n '%d' "'$c"
                (( n < 0 )) && (( n += 256 ))
                builtin printf -v c '%%%02X' "$n"
                out+="$c"
                ;;
        esac
    done
    builtin printf '\e]7;file://%s%s\e\\' "${HOSTNAME}" "$out"
}
# Reads the command's status, ends its mark and hands the status on unchanged, so a
# prompt command that shows it still sees the command's own. A prompt with no command
# before it also says D; Keepline only counts a D that follows a C.
_keepline_prompt() {
    builtin local status=$?
    builtin printf '\e]133;D;%s\e\\' "$status"
    _keepline_report_pwd
    return "$status"
}
# PS0 shows when a command line runs (bash 4.4 and later), not for an empty one; older
# bash never marks a start, so its commands are not heard.
PS0="${PS0-}\e]133;C\a"
# Ahead of the user's own prompt command, whether they keep it as a string or an array.
if [[ "$(builtin declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
    PROMPT_COMMAND=(_keepline_prompt "${PROMPT_COMMAND[@]}")
else
    PROMPT_COMMAND="_keepline_prompt${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi
