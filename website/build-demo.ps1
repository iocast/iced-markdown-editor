<#
.SYNOPSIS
Builds the web demo into static/demo for `zola serve`.
#>
$ErrorActionPreference = "Stop"

# Navigate to the examples/web directory relative to this script
Set-Location (Join-Path $PSScriptRoot "../examples/web")

# Build the web demo
trunk build --release --public-url /demo/ --dist ../../website/static/demo
