complete -c storm -s o -l output -d 'Output directory' -r
complete -c storm -s n -l name -d 'Override output filename' -r
complete -c storm -s s -l segments -d 'Number of segments (default: auto)' -r
complete -c storm -s c -l concurrent -d 'Max concurrent downloads' -r
complete -c storm -s l -l limit -d 'Bandwidth limit (e.g., 10MB/s)' -r
complete -c storm -l checksum -d 'Verify file against hash after download' -r
complete -c storm -s m -l mirror -d 'Additional mirror URLs' -r
complete -c storm -l completions -d 'Generate shell completions' -r -f -a "bash\t''
zsh\t''
fish\t''
powershell\t''"
complete -c storm -l gentle -d 'Conservative mode for sensitive servers'
complete -c storm -l no-resume -d 'Don\'t save resume manifest'
complete -c storm -l http1 -d 'Force HTTP/1.1'
complete -c storm -l http2 -d 'Force HTTP/2'
complete -c storm -l http3 -d 'Force HTTP/3'
complete -c storm -s q -l quiet -d 'Suppress progress output'
complete -c storm -s v -l verbose -d 'Detailed logging'
complete -c storm -s h -l help -d 'Print help'
complete -c storm -s V -l version -d 'Print version'
