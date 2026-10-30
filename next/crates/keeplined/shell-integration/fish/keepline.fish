# Keepline fish integration, sourced through --init-command after the user's own config:
# reports the working directory with OSC 7 (file://host/path, percent-encoded) so new
# tabs and splits open where you are.
set -e KEEPLINE_SHELL_INTEGRATION_DIR
function __keepline_report_pwd --on-event fish_prompt --on-variable PWD
    printf '\e]7;file://%s%s\e\\' $hostname (string escape --style=url -- $PWD)
end
