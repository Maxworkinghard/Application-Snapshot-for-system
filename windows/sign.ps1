<#
.SYNOPSIS
    Authenticode-sign the Windows build of AppSnapshot.

.DESCRIPTION
    Smart App Control (SAC) blocks unsigned dist\win-x64\AppSnapshot.exe and
    dist\win-arm64\AppSnapshot.exe. SAC allows a
    binary to run when either Microsoft's cloud intelligence rates it as safe, or the
    binary carries a valid signature from a certificate authority that participates in
    the Microsoft Trusted Root Program.

    Two constraints matter when choosing a certificate:

      1. Microsoft documents that the signature must chain to a CA in the Microsoft
         Trusted Root Program. Self-signed certificates are documented as not accepted.
      2. SAC's signature check is RSA only. Elliptic-curve (ECC) certificates are not
         supported, even when the chain is trusted.

    Self-signed signatures are documented as not accepted by SAC. Do not treat a local
    launch after -SelfTest as a distribution guarantee. Keep the RFC 3161 timestamp
    and validate on the target machine.

    This script covers the signing mechanics. Use -SelfTest to verify the toolchain
    end to end with a throwaway certificate before buying anything.

.PARAMETER PfxPath
    Path to a .pfx file holding the code signing certificate and its private key.

.PARAMETER PfxPassword
    Password for the .pfx file. Omit when the file is not password protected.

.PARAMETER Thumbprint
    Thumbprint of a code signing certificate already present in Cert:\CurrentUser\My
    or Cert:\LocalMachine\My.

.PARAMETER TimestampUrl
    RFC 3161 timestamp server. Defaults to DigiCert. Timestamping keeps signatures
    valid after the certificate expires.

.PARAMETER TargetPath
    File or directory to sign. Defaults to the dist folder next to this script.

.PARAMETER SelfTest
    Create a throwaway self-signed certificate, sign a copy of the build output in the
    temp folder, verify the signature, then remove everything. Proves the toolchain
    works without buying a certificate.

.EXAMPLE
    .\sign.ps1 -SelfTest

.EXAMPLE
    .\sign.ps1 -PfxPath C:\certs\codesign.pfx -PfxPassword YOUR_PFX_PASSWORD

.EXAMPLE
    .\sign.ps1 -Thumbprint 5A3B1C9D0E7F6A5B4C3D2E1F0099887766554433
#>
[CmdletBinding(DefaultParameterSetName = "Pfx")]
param(
    [Parameter(ParameterSetName = "Pfx", Mandatory = $true)]
    [string]$PfxPath,

    [Parameter(ParameterSetName = "Pfx")]
    [string]$PfxPassword,

    [Parameter(ParameterSetName = "Thumbprint", Mandatory = $true)]
    [string]$Thumbprint,

    [Parameter(ParameterSetName = "SelfTest", Mandatory = $true)]
    [switch]$SelfTest,

    [string]$TimestampUrl = "http://timestamp.digicert.com",

    [string]$TargetPath = (Join-Path $PSScriptRoot "dist")
)

$ErrorActionPreference = "Stop"

$CodeSigningEku = "1.3.6.1.5.5.7.3.3"

function Get-SignToolPath {
    $onPath = (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source
    if ($onPath) { return $onPath }

    $kits = "C:\Program Files (x86)\Windows Kits\10\bin"
    if (Test-Path -LiteralPath $kits) {
        $found = Get-ChildItem -LiteralPath $kits -Recurse -Filter "signtool.exe" -ErrorAction SilentlyContinue |
                 Where-Object { $_.FullName -match "\\x64\\" } |
                 Sort-Object FullName -Descending
        if ($found) { return $found[0].FullName }
    }
    return $null
}

function Get-TargetFiles {
    param([string]$Path)

    if (Test-Path -LiteralPath $Path -PathType Leaf) { return @(Get-Item -LiteralPath $Path) }
    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        throw "Target not found: $Path"
    }
    # Self-contained publish puts one AppSnapshot.exe per RID folder (win-x64 / win-arm64).
    $files = Get-ChildItem -LiteralPath $Path -Recurse -File |
             Where-Object { $_.Name -eq "AppSnapshot.exe" } |
             Sort-Object FullName
    if (-not $files) {
        $files = Get-ChildItem -LiteralPath $Path -File |
                 Where-Object { $_.Extension -in @(".exe", ".dll") } |
                 Sort-Object Name
    }
    if (-not $files) { throw "No AppSnapshot.exe found in $Path" }
    return @($files)
}

function Test-SigningCertificate {
    param([System.Security.Cryptography.X509Certificates.X509Certificate2]$Certificate)

    $problems = New-Object System.Collections.ArrayList
    $notices = New-Object System.Collections.ArrayList

    if (-not $Certificate.HasPrivateKey) {
        [void]$problems.Add("The certificate has no private key attached.")
    }

    $rsaKey = $null
    try {
        $rsaKey = [System.Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPublicKey($Certificate)
    } catch { }
    if (-not $rsaKey) {
        [void]$problems.Add("Smart App Control accepts RSA signatures only; this certificate is not RSA.")
    }

    if ($Certificate.NotAfter -lt (Get-Date)) {
        [void]$problems.Add("The certificate expired on $($Certificate.NotAfter.ToString('yyyy-MM-dd')).")
    }

    # Enumerating EnhancedKeyUsageList is unreliable on PowerShell 5.1: the items come
    # back with an empty ObjectId even when the extension is present. Read the 2.5.29.37
    # extension directly, and stay silent when it cannot be parsed rather than raising a
    # false alarm.
    $ekus = @()
    $ekuExtension = $Certificate.Extensions | Where-Object { $_.Oid.Value -eq "2.5.29.37" }
    if ($ekuExtension) {
        try {
            $parsed = New-Object System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension($ekuExtension, $false)
            $ekus = @($parsed.EnhancedKeyUsages | ForEach-Object { $_.Value })
        }
        catch {
            $ekus = @()
        }
    }
    if ($ekus.Count -gt 0 -and $ekus -notcontains $CodeSigningEku) {
        [void]$notices.Add("The certificate restricts its usage but omits the Code Signing EKU ($CodeSigningEku).")
    }

    $chain = New-Object System.Security.Cryptography.X509Certificates.X509Chain
    if (-not $chain.Build($Certificate)) {
        [void]$notices.Add("The chain does not validate against this machine's trust store. Smart App Control documents that it rejects such a signature, although a self-signed binary was observed to run on this build.")
    }

    return @{ Problems = @($problems); Notices = @($notices) }
}

# Native commands such as signtool write diagnostics to stderr. With
# $ErrorActionPreference set to Stop, PowerShell promotes the first stderr line into a
# terminating error, so a normal "untrusted chain" result would abort the script.
# This wrapper keeps native calls non-terminating and returns the exit code.
function Invoke-Native {
    param(
        [string]$FilePath,
        [string[]]$Arguments
    )

    $previous = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $FilePath @Arguments 2>&1
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previous
    }

    return @{ ExitCode = $exitCode; Output = @($output) }
}

function Invoke-Sign {
    param(
        [string]$SignTool,
        [string]$FilePath,
        [string[]]$SelectorArgs,
        [string[]]$SelectorValues,
        [string]$TimestampUrl
    )

    $arguments = @("sign", "/v", "/fd", "SHA256", "/td", "SHA256", "/tr", $TimestampUrl) +
                 $SelectorArgs + $SelectorValues + @($FilePath)

    return Invoke-Native -FilePath $SignTool -Arguments $arguments
}

function Get-SignatureReport {
    param([string]$SignTool, [string]$FilePath)

    $sig = Get-AuthenticodeSignature -LiteralPath $FilePath

    $verify = Invoke-Native -FilePath $SignTool -Arguments @("verify", "/pa", "/v", $FilePath)
    $trusted = ($verify.ExitCode -eq 0)

    return @{
        HasSignature = ($null -ne $sig.SignerCertificate)
        Signer       = $sig.SignerCertificate
        Trusted      = $trusted
        Status       = $sig.Status
        Timestamped  = ($null -ne $sig.TimeStamperCertificate)
        Output       = $verify.Output
    }
}

# ---------------------------------------------------------------- main

$signTool = Get-SignToolPath
if (-not $signTool) {
    throw "signtool.exe was not found. Install the Windows SDK, or run this from a Visual Studio Developer prompt."
}
Write-Host "signtool : $signTool"

$selfSignedRoot = $null
$testCopy = $null

try {
    if ($SelfTest) {
        Write-Host ""
        Write-Host "Self test: creating a throwaway self-signed certificate." -ForegroundColor Yellow
        Write-Host "The chain will not be trusted, which is expected for a self-signed certificate."

        $subject = "CN=AppSnapshot SelfTest"
        $selfSignedRoot = New-SelfSignedCertificate `
            -Type CodeSigningCert `
            -Subject $subject `
            -CertStoreLocation "Cert:\CurrentUser\My" `
            -KeyAlgorithm RSA `
            -KeyLength 2048 `
            -KeyUsage DigitalSignature `
            -NotAfter (Get-Date).AddDays(1)

        $selectorArgs = @("/sha1")
        $selectorValues = @(($selfSignedRoot.Thumbprint -replace "\s", ""))
        $certificate = $selfSignedRoot

        $source = Join-Path $TargetPath "AppSnapshot.exe"
        if (-not (Test-Path -LiteralPath $source)) {
            $source = Join-Path $TargetPath "win-x64\AppSnapshot.exe"
        }
        if (-not (Test-Path -LiteralPath $source)) {
            if (Test-Path -LiteralPath $TargetPath -PathType Container) {
                $first = Get-ChildItem -LiteralPath $TargetPath -Recurse -File -Filter "AppSnapshot.exe" |
                         Select-Object -First 1
                if ($first) { $source = $first.FullName }
            }
        }
        if (-not (Test-Path -LiteralPath $source)) {
            throw "Nothing to sign for the self test. Build first, or pass -TargetPath."
        }

        $testCopy = Join-Path $env:TEMP ("AppSnapshot.selftest." + [Guid]::NewGuid().ToString("N") + ".exe")
        Copy-Item -LiteralPath $source -Destination $testCopy -Force
        Write-Host "Test copy: $testCopy"
        $files = @(Get-Item -LiteralPath $testCopy)
    }
    elseif ($PfxPath) {
        if (-not (Test-Path -LiteralPath $PfxPath)) { throw "PFX not found: $PfxPath" }

        $flags = [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::EphemeralKeySet
        $certificate = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($PfxPath, $PfxPassword, $flags)
        Write-Host "Certificate: $($certificate.Subject)"

        $selectorArgs = @("/f", $PfxPath)
        $selectorValues = @()
        if ($PfxPassword) { $selectorArgs += "/p"; $selectorValues = @($PfxPassword) }
        $files = Get-TargetFiles -Path $TargetPath
    }
    else {
        $normalized = $Thumbprint -replace "\s", ""
        $certificate = @(Get-ChildItem Cert:\CurrentUser\My, Cert:\LocalMachine\My -ErrorAction SilentlyContinue |
                         Where-Object { $_.Thumbprint -eq $normalized }) | Select-Object -First 1
        if (-not $certificate) { throw "No certificate with thumbprint $normalized was found in the store." }
        Write-Host "Certificate: $($certificate.Subject)"

        $selectorArgs = @("/sha1")
        $selectorValues = @($normalized)
        $files = Get-TargetFiles -Path $TargetPath
    }

    Write-Host ""
    $check = Test-SigningCertificate -Certificate $certificate
    foreach ($n in $check.Notices) { Write-Host "NOTICE   $n" -ForegroundColor Yellow }
    foreach ($p in $check.Problems) { Write-Host "PROBLEM  $p" -ForegroundColor Red }
    if ($check.Problems.Count -gt 0) {
        throw "The certificate cannot be used for signing. Fix the problems above and retry."
    }

    $signed = 0
    $trustedCount = 0
    $untrustedCount = 0
    $failed = @()

    foreach ($file in $files) {
        Write-Host ""
        Write-Host "Signing  $($file.Name)  [$([math]::Round($file.Length / 1KB, 1)) KB]"

        $result = Invoke-Sign -SignTool $signTool -FilePath $file.FullName `
                              -SelectorArgs $selectorArgs -SelectorValues $selectorValues `
                              -TimestampUrl $TimestampUrl

        if ($result.ExitCode -ne 0) {
            Write-Host "  sign failed (exit $($result.ExitCode))" -ForegroundColor Red
            foreach ($line in $result.Output) { Write-Host "    $line" -ForegroundColor DarkGray }
            $failed += $file.Name
            continue
        }

        $report = Get-SignatureReport -SignTool $signTool -FilePath $file.FullName

        if (-not $report.HasSignature) {
            Write-Host "  no signature found on the file after signing" -ForegroundColor Red
            $failed += $file.Name
            continue
        }

        $signed++
        if ($report.Timestamped) { Write-Host "  timestamped" }

        if ($report.Trusted) {
            Write-Host "  signature verified against the local trust store" -ForegroundColor Green
            $trustedCount++
        }
        else {
            Write-Host "  signature applied; chain is NOT trusted ($($report.Status))" -ForegroundColor Yellow
            $untrustedCount++
        }
    }

    Write-Host ""
    Write-Host "Signed: $signed / $($files.Count)    trusted chain: $trustedCount    untrusted chain: $untrustedCount"
    if ($failed.Count -gt 0) { Write-Host "Failed: $($failed -join ', ')" -ForegroundColor Red }

    if ($SelfTest) {
        Write-Host ""
        if ($failed.Count -eq 0) {
            Write-Host "Self test complete. The signing toolchain works." -ForegroundColor Green
        }
        Write-Host ""
        Write-Host "Microsoft documents that SAC requires a certificate in the Microsoft Trusted Root"
        Write-Host "Program. A self-signed -SelfTest copy is only for toolchain verification."
        Write-Host "Keep the timestamp enabled, and do not assume a self-signed binary will run"
        Write-Host "on another machine."
    }
}
finally {
    if ($selfSignedRoot) {
        Remove-Item -LiteralPath ("Cert:\CurrentUser\My\" + $selfSignedRoot.Thumbprint) -Force -ErrorAction SilentlyContinue
        Write-Host "Removed the throwaway certificate."
    }
    if ($testCopy -and (Test-Path -LiteralPath $testCopy)) {
        Remove-Item -LiteralPath $testCopy -Force -ErrorAction SilentlyContinue
        Write-Host "Removed the test copy."
    }
}

# signtool leaves its own exit code behind, so set ours explicitly for calling scripts
# and CI pipelines.
if ($failed -and $failed.Count -gt 0) {
    exit 1
}
exit 0
