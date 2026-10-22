#!/usr/bin/env pwsh

# Cross-platform WezTerm shell integration for PowerShell.
# Features:
#  - OSC 133 (semantic prompt A/C/D)
#  - OSC 7 (cwd)
#  - OSC 1337 user-vars: WEZTERM_PROG (full command line), WEZTERM_CMD (resolved command),
#    WEZTERM_USER, WEZTERM_HOST, WEZTERM_IN_TMUX
#  - Toggleable via functions (enable/disable)

# Create a new dynamic module so we don't pollute the global namespace
# with our functions and variables
$null = New-Module wezterm {
    # --------------------------
    # Configurable on/off flags
    # --------------------------
    # Defaults: everything on
    $script:Wez_EnableSemanticZones = $true
    $script:Wez_EnableCwd           = $true
    $script:Wez_EnableUserVars      = $true

    # --------------------------
    # Environment detection
    # --------------------------
    function Test-IsWezTerm {
      if ($env:WEZTERM_FORCE -eq '1') { return $true }
      return ($env:TERM_PROGRAM -like 'WezTerm*' -or $env:WEZTERM_PANE)
    }

    if (-not (Test-IsWezTerm)) { return }

    # --------------------------
    # Helpers (portable)
    # --------------------------
    $script:ESC = [char]27
    $script:BEL = [char]7
    $script:ST  = "$([char]27)\"   # ESC \  (String Terminator)

    function Get-HostNamePortable {
      if ($env:COMPUTERNAME) { return $env:COMPUTERNAME }
      if ($env:HOSTNAME)     { return $env:HOSTNAME }
      return [System.Net.Dns]::GetHostName()
    }

    function Get-UserPortable {
      if ($env:USERNAME) { return $env:USERNAME }
      if ($env:USER)     { return $env:USER }
      try { (whoami) } catch { "" }
    }

    # --------------------------
    # Low-level OSC writers
    # --------------------------
    function Write-OSC133([string]$Code) {
      [Console]::Out.Write("$($script:ESC)]133;$Code$($script:BEL)")
    }
    function Write-OSC1337([string]$Code) {
      [Console]::Out.Write("$($script:ESC)]1337;$Code$($script:BEL)")
    }
    function Write-OSC7([string]$Path) {
      if (-not $script:Wez_EnableCwd) { return }
      try { $abs = (Resolve-Path -LiteralPath $Path).Path } catch { $abs = (Get-Location).Path }

      # We follow WezTerm’s own examples: file://HOST/ABSOLUTE/PATH using "/" separators.
      # (percent-encoding is optional for typical paths)
      $wezhost = Get-HostNamePortable
      $posix = ($abs -replace '\\','/').TrimStart('/')
      [Console]::Out.Write("$($script:ESC)]7;file://$wezhost/$posix$($script:ST)")
    }

    function Set-UserVar([string]$Name, [string]$Value) {
      if (-not $script:Wez_EnableUserVars) { return }
      $b64 = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($Value))
      Write-OSC1337 "SetUserVar=$Name=$b64"
    }

    # --------------------------
    # Command resolution helper
    # --------------------------
    function Get-CurrentLineAndCommand {
      # Returns [pscustomobject]@{ Line = "..."; Command = "resolved-command" }
      $tokens = $null
      try {
        [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState(
          [ref]$null, [ref]$tokens, [ref]$null, [ref]$null
        )
      } catch {
          Write-Error "Failed to get PSReadLine buffer state: $_"
      }
      $resolved = ""
      try {
        # Find the first command element if possible
        $firstCmd = $tokens | Where-Object Kind -In Identifier,Generic | Select-Object -First 1
        if ($firstCmd) {
          $candidate = $firstCmd.Text
          $gc = Get-Command -Name $candidate -ErrorAction SilentlyContinue
          if ($gc) {
            # For aliases show the definition
            if ($gc.CommandType -eq 'Alias') {
              $resolved = $gc.Definition
            } else {
              $resolved = $gc.Name -replace '(?i)\.exe$', ''
            }
          } else {
            $resolved = $candidate
          }
        }
      } catch {
          Write-Error "Failed to resolve command: $_"
      }

      [pscustomobject]@{
        Line    = "$tokens"
        Command = "$resolved"
      }
    }

    # --------------------------
    # Semantic prompt marks
    # --------------------------
    function Write-WeztermSequences([int]$ExitCode, [int]$P_id) {
      if ($script:Wez_EnableSemanticZones) {
        # 1) close the previous command zone with its exit status
        Write-OSC133 ("D;{0};aid={1}" -f $ExitCode, $P_id)
        # 2) start the new prompt zone
        Write-OSC133 "A"
      }

      # 3) update OSC 7 cwd
      Write-OSC7 (Get-Location).Path

      # 4) refresh user vars typically shown in status/tab title
      if ($script:Wez_EnableUserVars) {
        Set-UserVar "WEZTERM_USER"   (Get-UserPortable)
        Set-UserVar "WEZTERM_HOST"   (Get-HostNamePortable)
        Set-UserVar "WEZTERM_IN_TMUX" ($(if ($env:TMUX) { "1" } else { "0" }))
        # Clear these until we actually run a command on Enter:
        Set-UserVar "WEZTERM_PROG" ""
        Set-UserVar "WEZTERM_CMD"  ""
      }

      Write-OSC1337 "ShellIntegrationVersion=15;shell=pwsh"
    }

    if ($Env:STARSHIP_SESSION_KEY) {
      # Starship hook: called before prompt renders (if defined)
      function global:Invoke-Starship-PreCommand {
        $status =
          if ($null -ne $global:LASTEXITCODE -and $global:LASTEXITCODE -ne 0) {
            [int]$global:LASTEXITCODE
          } elseif ($global:? -eq $false) { 1 } else { 0 }

        Write-WeztermSequences -ExitCode $status -P_id $PID
      }
    } else {
      $script:Wez_OriginalPrompt = $function:Prompt

      function global:prompt {
        # This prompt override is heavily inspired by Starship's PowerShell init script:
        # https://github.com/starship/starship/blob/master/src/init/starship.ps1
        $origDollarQuestion = $global:?
        $origLastExitCode   = $global:LASTEXITCODE

        # We start from the premise that the command executed correctly, which covers also the fresh console.
        $lastExitCodeForPrompt = 0
        if ($lastCmd = Get-History -Count 1) {
            # In case we have a False on the Dollar hook, we know there's an error.
            if (-not $origDollarQuestion) {
                # We retrieve the InvocationInfo from the most recent error using $global:error[0]
                $lastCmdletError = try { $global:error[0] |  Where-Object { $_ -ne $null } | Select-Object -ExpandProperty InvocationInfo } catch { $null }
                # We check if the last command executed matches the line that caused the last error, in which case we know
                # it was an internal Powershell command, otherwise, there MUST be an error code.
                $lastExitCodeForPrompt = if ($null -ne $lastCmdletError -and $lastCmd.CommandLine -eq $lastCmdletError.Line) { 1 } else { $origLastExitCode }
            }
        }

        # WezTerm: mark command end, prompt start, cwd, and user-vars
        Write-WeztermSequences -ExitCode $lastExitCodeForPrompt -P_id $PID

        # Render the previously saved prompt
        $promptText = & $script:Wez_OriginalPrompt
        $promptText

        # Propagate the original $LASTEXITCODE from before the prompt function was invoked.
        $global:LASTEXITCODE = $origLastExitCode

        # Propagate the original $? automatic variable value from before the prompt function was invoked.
        #
        # $? is a read-only or constant variable so we can't directly override it.
        # In order to propagate up its original boolean value we will take an action
        # which will produce the desired value.
        #
        # This has to be the very last thing that happens in the prompt function
        # since every PowerShell command sets the $? variable.
        if ($global:? -ne $origDollarQuestion) {
            if ($origDollarQuestion) {
                 # Simple command which will execute successfully and set $? = True without any other side affects.
                1+1
            } else {
                # Write-Error will set $? to False.
                # ErrorAction Ignore will prevent the error from being added to the $Error collection.
                Write-Error '' -ErrorAction 'Ignore'
            }
        }
      }
    }

    # --------------------------
    # PSReadLine Enter wrapper
    # --------------------------
    $script:Wez_EnterBindingInstalled = $false
    function Install-EnterBinding {
      if ($script:Wez_EnterBindingInstalled) { return }
      try {
        $existing = Get-PSReadLineKeyHandler -Bound | Where-Object { $_.Key -eq 'Enter' }
      } catch { $existing = $null }

      # Only override the default AcceptLine to avoid stomping custom setups
      if ($null -eq $existing -or $existing.Function -eq 'AcceptLine') {
        Set-PSReadLineKeyHandler -Key Enter -ScriptBlock {
          param($key,$arg)
          try {
            $info = Get-CurrentLineAndCommand
            # Set both "full command line" and "resolved command" user vars
            if ($script:Wez_EnableUserVars) {
              Set-UserVar "WEZTERM_PROG" $info.Line
              Set-UserVar "WEZTERM_CMD"  $info.Command
            }
            # Mark end of input/start of output
            if ($script:Wez_EnableSemanticZones) { Write-OSC133 "C" }
          } catch {
              Write-Error "Failed in Enter binding: $_"
          }
          [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
        } -Description "WezTerm: OSC 133 C + set user vars before executing"
        $script:Wez_EnterBindingInstalled = $true
      }
    }
    function Uninstall-EnterBinding {
      if (-not $script:Wez_EnterBindingInstalled) { return }
      # Restore default AcceptLine
      Set-PSReadLineKeyHandler -Key Enter -Function AcceptLine
      $script:Wez_EnterBindingInstalled = $false
    }

    # --------------------------
    # Public toggles
    # --------------------------
    function Enable-Wezterm-Integration    { Enable-Wezterm-SemanticZones; Enable-Wezterm-Cwd; Enable-Wezterm-UserVars; }
    function Disable-Wezterm-Integration   { Disable-Wezterm-SemanticZones; Disable-Wezterm-Cwd; Disable-Wezterm-UserVars; }

    function Enable-Wezterm-SemanticZones  { $script:Wez_EnableSemanticZones = $true;  Install-EnterBinding }
    function Disable-Wezterm-SemanticZones { $script:Wez_EnableSemanticZones = $false; Uninstall-EnterBinding }

    function Enable-Wezterm-Cwd            { $script:Wez_EnableCwd = $true; }
    function Disable-Wezterm-Cwd           { $script:Wez_EnableCwd = $false; }

    function Enable-Wezterm-UserVars       { $script:Wez_EnableUserVars = $true }
    function Disable-Wezterm-UserVars      { $script:Wez_EnableUserVars = $false }

    Export-ModuleMember -Function @(
        "Enable-Wezterm-Integration",
        "Disable-Wezterm-Integration",
        "Enable-Wezterm-SemanticZones",
        "Disable-Wezterm-SemanticZones",
        "Enable-Wezterm-Cwd",
        "Disable-Wezterm-Cwd",
        "Enable-Wezterm-UserVars",
        "Disable-Wezterm-UserVars"
    )
}
