param([string]$Exe, [string]$Title = "Form")
# Reads the form example through UI Automation, and flips its switch with
# the toggle pattern, as assistive tools do. No key presses are sent.
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$T = [System.Windows.Automation.TreeScope]
$All = [System.Windows.Automation.Condition]::TrueCondition

function Show-Tree($e, $depth) {
  $info = $e.Current
  $extra = ""
  try { $extra += " toggle=" + $e.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Current.ToggleState } catch {}
  try { $extra += " value='" + $e.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value + "'" } catch {}
  ("  " * $depth) + $info.ControlType.ProgrammaticName.Replace("ControlType.", "") + " '" + $info.Name + "'" + $extra
  foreach ($child in $e.FindAll($T::Children, $All)) { Show-Tree $child ($depth + 1) }
}

function Find-Named($root, $name) {
  foreach ($e in $root.FindAll($T::Descendants, $All)) { if ($e.Current.Name -eq $name) { return $e } }
  return $null
}

$p = Start-Process -FilePath $Exe -PassThru -NoNewWindow
try {
  $byProcess = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, $p.Id)
  $window = $null
  for ($i = 0; $i -lt 100 -and -not $window; $i++) {
    Start-Sleep -Milliseconds 100
    foreach ($w in $A::RootElement.FindAll($T::Children, $byProcess)) { if ($w.Current.Name -eq $Title) { $window = $w } }
  }
  if (-not $window) { "FAIL  no window called '$Title'"; return }
  $isEdit = New-Object System.Windows.Automation.PropertyCondition($A::ControlTypeProperty, [System.Windows.Automation.ControlType]::Edit)
  $edits = $null
  for ($i = 0; $i -lt 30; $i++) { Start-Sleep -Milliseconds 100; $edits = $window.FindAll($T::Descendants, $isEdit); if ($edits.Count -ge 2) { break } }
  "--- the tree"
  Show-Tree $window 0
  "--- checks"
  $isSwitch = $null
  foreach ($e in $window.FindAll($T::Descendants, $All)) { if ($e.Current.Name -eq "Send me updates") { $isSwitch = $e } }
  if (-not $isSwitch) { "FAIL  no switch"; return }
  $toggle = $isSwitch.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
  "switch before: " + $toggle.Current.ToggleState
  $toggle.Toggle(); Start-Sleep -Milliseconds 500
  "switch after Toggle: " + $toggle.Current.ToggleState
} finally {
  Stop-Process -Id $p.Id -ErrorAction SilentlyContinue
}
