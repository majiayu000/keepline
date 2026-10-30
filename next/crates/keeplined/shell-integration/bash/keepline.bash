# Keepline bash integration. Keepline starts interactive bash with --init-file pointing
# here, which bash reads instead of ~/.bashrc; so this file first loads the user's own
# startup files the way bash itself would, then reports the working directory with
# OSC 7 (file://host/path, percent-encoded) so new tabs and splits open where you are.
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
# Ahead of the user's own prompt command, whether they keep it as a string or an array.
if [[ "$(builtin declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
    PROMPT_COMMAND=(_keepline_report_pwd "${PROMPT_COMMAND[@]}")
else
    PROMPT_COMMAND="_keepline_report_pwd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi
