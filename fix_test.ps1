param([string]$FilePath)

$content = Get-Content $FilePath -Raw

# Count total upsert_token calls
$upsertCalls = [regex]::Matches($content, '\.upsert_token\(').Count
Write-Output "Total upsert_token() calls: $upsertCalls"

# Fix upsert_token calls that are missing SessionId::nil()
# Pattern: .upsert_token( ... NewTokenSpec { ... }, \n             N,\n        )
# The issue is the closing pattern after the turn number
$pattern = '(\.upsert_token\(\s*\n\s+NewTokenSpec\s*\{)((?:[^{}]|\{[^{}]*\})*?)(\})\s*,\s*\n\s+)(\d+)(,\s*\n\s+\)\s*\n\s+\.await)'

$fixed = [regex]::Replace($content, $pattern, {
    param($m)
    # Check if the closing part already has SessionId::nil()
    $closing = $m.Groups[4].Value + $m.Groups[5].Value
    $afterTurn = $m.Value.Substring($m.Length - $m.Groups[5].Length - $m.Groups[4].Length)
    if ($afterTurn -match 'SessionId::nil') {
        return $m.Value  # Already fixed
    }
    # Add SessionId::nil()
    $before = $m.Groups[1].Value  # .upsert_token(\n    NewTokenSpec {
    $body = $m.Groups[2].Value    # content of NewTokenSpec
    $closeBrace = $m.Groups[3].Value  # }
    $commaNewline = $m.Groups[4].Value  # N
    $turnNum = $m.Groups[5].Value  # ,
    $rest = $m.Groups[6].Value  # \n        )\n        .await
    
    return "$before$body$closeBrace,$commaNewline$turnNum,$rest`n            SessionId::nil(),"
})

# Now fix explore_memory calls with only 5 args
$fixed2 = [regex]::Replace($fixed, '(explore_memory\("all", "", 0, 10, 0\))\.await', {
    param($m)
    "$($m.Groups[1].Value), SessionId::nil()).await"
})

# Count changes
$oldCount = [regex]::Matches($content, 'SessionId::nil\(\)').Count
$newCount = [regex]::Matches($fixed2, 'SessionId::nil\(\)').Count
Write-Output "SessionId::nil() count: $oldCount -> $newCount"
Write-Output "Added $($newCount - $oldCount) references"

# Try to verify the file still parses as valid Rust-ish
$openParens = [regex]::Matches($fixed2, '\(').Count
$closeParens = [regex]::Matches($fixed2, '\)').Count
$openBraces = [regex]::Matches($fixed2, '\{').Count
$closeBraces = [regex]::Matches($fixed2, '\}').Count
Write-Output "Parentheses balance: $openParens open, $closeParens close (diff: $($openParens - $closeParens))"
Write-Output "Brace balance: $openBraces open, $closeBraces close (diff: $($openBraces - $closeBraces))"

if (($openParens -eq $closeParens) -and ($openBraces -eq $closeBraces)) {
    Write-Output "PASS: Balanced parens/braces"
} else {
    Write-Output "WARNING: Imbalanced parens/braces!"
}

Set-Content $FilePath -Value $fixed2 -NoNewline -Encoding utf8
Write-Output "File written: $FilePath"
