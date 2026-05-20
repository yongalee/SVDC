param(
    [switch]$DryRun
)

$repoDir = "c:\Users\yonga\TestWork\SVDC"
Set-Location -Path $repoDir

Write-Host "============================================="
Write-Host "SVDC Remote Monitoring & Testing Automation"
Write-Host "Current Time: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
if ($DryRun) { Write-Host "Mode: DRY RUN (Simulated Verification)" }
Write-Host "============================================="

# 1. Fetch from remote
Write-Host "[1/5] Fetching from remote repository..."
git fetch origin
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to fetch from remote origin."
    exit 1
}

# Get current branch
$branch = git branch --show-current
if ([string]::IsNullOrEmpty($branch)) { $branch = "master" }
$remoteBranch = "origin/$branch"

Write-Host "Active branch is: $branch (tracking $remoteBranch)"

# 2. Check for remote commits
$localCommit = git rev-parse HEAD
$remoteCommit = git rev-parse $remoteBranch

Write-Host "Local commit : $localCommit"
Write-Host "Remote commit: $remoteCommit"

if ($localCommit -eq $remoteCommit) {
    Write-Host "No new commits on remote. Workspace is up-to-date."
    if (-not $DryRun) {
        exit 0
    } else {
        Write-Host "[DryRun] Proceeding with verification anyway..."
    }
} else {
    Write-Host "New remote commits detected! Pulling changes..."
    if (-not $DryRun) {
        git pull origin $branch
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Failed to pull changes from remote."
            exit 1
        }
    } else {
        Write-Host "[DryRun] Simulating git pull..."
    }
}

# 3. Code Verification Pipeline
Write-Host "[2/5] Running Cargo Code Verification Pipeline..."

# Format Check
Write-Host "-> Running cargo fmt check..."
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    Write-Error "Verification Failed: cargo fmt issues found. Skipping debug export & commit."
    exit 1
}

# Clippy Check
Write-Host "-> Running cargo clippy..."
cargo clippy --workspace -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Error "Verification Failed: cargo clippy warnings or errors found. Skipping debug export & commit."
    exit 1
}

# Test Check
Write-Host "-> Running cargo test..."
cargo test --workspace
if ($LASTEXITCODE -ne 0) {
    Write-Error "Verification Failed: cargo test failed. Skipping debug export & commit."
    exit 1
}

Write-Host "Verification pipeline passed successfully."

# 4. Build and Update Debug Export Folder
Write-Host "[3/5] Compiling and updating debug export directory..."
cargo build
if ($LASTEXITCODE -ne 0) {
    Write-Error "Cargo build failed."
    exit 1
}

# Re-create debug folder if deleted
if (-not (Test-Path "debug")) {
    New-Item -ItemType Directory -Path "debug" -Force | Out-Null
}

# Copy compiled binaries (ssiec-sv-publisher.exe, svdc.exe)
$filesCopied = $false
$pubPath = "target\debug\ssiec-sv-publisher.exe"
$svdcPath = "target\debug\svdc.exe"

if (Test-Path $pubPath) {
    Copy-Item -Path $pubPath -Destination "debug\" -Force
    Write-Host "Copied ssiec-sv-publisher.exe to debug/"
    $filesCopied = $true
} else {
    Write-Warning "Could not find ssiec-sv-publisher.exe"
}

if (Test-Path $svdcPath) {
    Copy-Item -Path $svdcPath -Destination "debug\" -Force
    Write-Host "Copied svdc.exe to debug/"
    $filesCopied = $true
} else {
    Write-Warning "Could not find svdc.exe"
}

# 5. Commit and Push back if updated
Write-Host "[4/5] Checking for debug output modifications..."
$status = git status --porcelain debug/

if (-not [string]::IsNullOrEmpty($status)) {
    Write-Host "Debug binaries have changed. Committing updates..."
    if (-not $DryRun) {
        git add debug/
        git commit -m "WBS-1.4: automated build and verification loop updates" -m "Verify Claude Code's latest push and compile updated binaries to the debug folder." -m "Agent: antigravity-subagent-monitor"
        
        Write-Host "[5/5] Pushing changes back to remote origin..."
        git push origin $branch
        if ($LASTEXITCODE -ne 0) {
            Write-Warning "Git push failed. It could be due to credentials or lock, but changes are committed locally."
        } else {
            Write-Host "Updates pushed to remote successfully!"
        }
    } else {
        Write-Host "[DryRun] Simulating git commit and push back..."
    }
} else {
    Write-Host "No changes in debug binaries. Skipping commit/push."
}

Write-Host "SVDC remote monitoring and test run finished successfully."
