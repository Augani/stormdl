
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'storm' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'storm'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'storm' {
            [CompletionResult]::new('-o', '-o', [CompletionResultType]::ParameterName, 'Output directory')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'Output directory')
            [CompletionResult]::new('-n', '-n', [CompletionResultType]::ParameterName, 'Override output filename')
            [CompletionResult]::new('--name', '--name', [CompletionResultType]::ParameterName, 'Override output filename')
            [CompletionResult]::new('-s', '-s', [CompletionResultType]::ParameterName, 'Number of segments (default: auto)')
            [CompletionResult]::new('--segments', '--segments', [CompletionResultType]::ParameterName, 'Number of segments (default: auto)')
            [CompletionResult]::new('-c', '-c', [CompletionResultType]::ParameterName, 'Max concurrent downloads')
            [CompletionResult]::new('--concurrent', '--concurrent', [CompletionResultType]::ParameterName, 'Max concurrent downloads')
            [CompletionResult]::new('-l', '-l', [CompletionResultType]::ParameterName, 'Bandwidth limit (e.g., 10MB/s)')
            [CompletionResult]::new('--limit', '--limit', [CompletionResultType]::ParameterName, 'Bandwidth limit (e.g., 10MB/s)')
            [CompletionResult]::new('--checksum', '--checksum', [CompletionResultType]::ParameterName, 'Verify file against hash after download')
            [CompletionResult]::new('-m', '-m', [CompletionResultType]::ParameterName, 'Additional mirror URLs')
            [CompletionResult]::new('--mirror', '--mirror', [CompletionResultType]::ParameterName, 'Additional mirror URLs')
            [CompletionResult]::new('--completions', '--completions', [CompletionResultType]::ParameterName, 'Generate shell completions')
            [CompletionResult]::new('--gentle', '--gentle', [CompletionResultType]::ParameterName, 'Conservative mode for sensitive servers')
            [CompletionResult]::new('--no-resume', '--no-resume', [CompletionResultType]::ParameterName, 'Don''t save resume manifest')
            [CompletionResult]::new('--http1', '--http1', [CompletionResultType]::ParameterName, 'Force HTTP/1.1')
            [CompletionResult]::new('--http2', '--http2', [CompletionResultType]::ParameterName, 'Force HTTP/2')
            [CompletionResult]::new('--http3', '--http3', [CompletionResultType]::ParameterName, 'Force HTTP/3')
            [CompletionResult]::new('-q', '-q', [CompletionResultType]::ParameterName, 'Suppress progress output')
            [CompletionResult]::new('--quiet', '--quiet', [CompletionResultType]::ParameterName, 'Suppress progress output')
            [CompletionResult]::new('-v', '-v', [CompletionResultType]::ParameterName, 'Detailed logging')
            [CompletionResult]::new('--verbose', '--verbose', [CompletionResultType]::ParameterName, 'Detailed logging')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
