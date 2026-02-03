#compdef storm

autoload -U is-at-least

_storm() {
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext="$curcontext" state line
    _arguments "${_arguments_options[@]}" : \
'-o+[Output directory]:OUTPUT:_default' \
'--output=[Output directory]:OUTPUT:_default' \
'-n+[Override output filename]:NAME:_default' \
'--name=[Override output filename]:NAME:_default' \
'-s+[Number of segments (default\: auto)]:SEGMENTS:_default' \
'--segments=[Number of segments (default\: auto)]:SEGMENTS:_default' \
'-c+[Max concurrent downloads]:CONCURRENT:_default' \
'--concurrent=[Max concurrent downloads]:CONCURRENT:_default' \
'-l+[Bandwidth limit (e.g., 10MB/s)]:LIMIT:_default' \
'--limit=[Bandwidth limit (e.g., 10MB/s)]:LIMIT:_default' \
'--checksum=[Verify file against hash after download]:CHECKSUM:_default' \
'*-m+[Additional mirror URLs]:MIRRORS:_default' \
'*--mirror=[Additional mirror URLs]:MIRRORS:_default' \
'--completions=[Generate shell completions]:COMPLETIONS:(bash zsh fish powershell)' \
'--gentle[Conservative mode for sensitive servers]' \
'--no-resume[Don'\''t save resume manifest]' \
'--http1[Force HTTP/1.1]' \
'--http2[Force HTTP/2]' \
'--http3[Force HTTP/3]' \
'-q[Suppress progress output]' \
'--quiet[Suppress progress output]' \
'-v[Detailed logging]' \
'--verbose[Detailed logging]' \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
'::url -- URL to download:_default' \
&& ret=0
}

(( $+functions[_storm_commands] )) ||
_storm_commands() {
    local commands; commands=()
    _describe -t commands 'storm commands' commands "$@"
}

if [ "$funcstack[1]" = "_storm" ]; then
    _storm "$@"
else
    compdef _storm storm
fi
