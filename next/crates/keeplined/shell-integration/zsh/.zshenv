# Keepline zsh integration. Keepline starts zsh with ZDOTDIR pointing here; this file
# puts the user's ZDOTDIR back first, so .zprofile, .zshrc and .zlogin load as usual.
if [[ -n "${KEEPLINE_ZSH_ZDOTDIR+X}" ]]; then
    'builtin' 'export' ZDOTDIR="$KEEPLINE_ZSH_ZDOTDIR"
    'builtin' 'unset' 'KEEPLINE_ZSH_ZDOTDIR'
else
    'builtin' 'unset' 'ZDOTDIR'
fi

{
    'builtin' 'typeset' _keepline_file="${ZDOTDIR-$HOME}/.zshenv"
    [[ ! -r "$_keepline_file" ]] || 'builtin' 'source' '--' "$_keepline_file"
} always {
    if [[ -o 'interactive' ]]; then
        'builtin' 'typeset' _keepline_file="${KEEPLINE_SHELL_INTEGRATION_DIR}/zsh/keepline-integration"
        [[ ! -r "$_keepline_file" ]] || 'builtin' 'source' '--' "$_keepline_file"
    fi
    'builtin' 'unset' '_keepline_file' 'KEEPLINE_SHELL_INTEGRATION_DIR'
}
