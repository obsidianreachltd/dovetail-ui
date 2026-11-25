param([string]$Exe, [string]$Title = "Counter")
# Reads the counter example through UI Automation, as Narrator does, and
# drives it through UI Automation only: no key presses are sent.
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$T = [System.Windows.Automation.TreeScope]
$All = [System.Windows.Automation.Condition]::TrueCondition

function Show-Tree($e, $depth) {
  if ($depth -gt 8) { return }
  $c = $e.Current
  $extra = ""
  try {
    $toggle = $e.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
    $extra += " toggle=" + $toggle.Current.ToggleState
  } catch {}
  try {
    $value = $e.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    $extra += " value='" + $value.Current.Value + "'"
  } catch {}
  ("  " * $depth) + $c.ControlType.ProgrammaticName.Replace("ControlType.", "") + " '" + $c.Name + "'" + $extra
  foreach ($child in $e.FindAll($T::Children, $All)) { Show-Tree $child ($depth + 1) }
}

function Find-Named($root, $name) {
  foreach ($e in $root.FindAll($T::Descendants, $All)) { if ($e.Current.Name -eq $name) { return $e } }
  return $null
}

$p = Start-Process -FilePath $Exe -PassThru -NoNewWindow
try {
  # The window, by process and title, among the desktop's windows.
  $byProcess = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, $p.Id)
  $window = $null
  for ($i = 0; $i -lt 100 -and -not $window; $i++) {
    Start-Sleep -Milliseconds 100
    foreach ($w in $A::RootElement.FindAll($T::Children, $byProcess)) { if ($w.Current.Name -eq $Title) { $window = $w } }
  }
  if (-not $window) { "FAIL  no window called '$Title'"; return }
  "window: '" + $window.Current.Name + "' class " + $window.Current.ClassName + ", framework '" + $window.Current.FrameworkId + "'"
  # The first request starts AccessKit; the tree follows on the next frame.
  $button = $null
  for ($i = 0; $i -lt 30 -and -not $button; $i++) { Start-Sleep -Milliseconds 100; $button = Find-Named $window "Increment" }
  "--- the tree"
  Show-Tree $window 0
  if (-not $button) { "FAIL  no Increment button"; return }
  "--- checks"
  "button type: " + $button.Current.ControlType.ProgrammaticName + ", focusable: " + $button.Current.IsKeyboardFocusable
  $invoke = $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
  $invoke.Invoke(); Start-Sleep -Milliseconds 300; $invoke.Invoke(); Start-Sleep -Milliseconds 500
  $count = Find-Named $window "Count: 2"
  if ($count) { "PASS  pressing through UI Automation twice shows 'Count: 2'" } else { "FAIL  the count did not become 2" }
  $button.SetFocus(); Start-Sleep -Milliseconds 500
  $focused = $A::FocusedElement
  "focused after SetFocus: " + $focused.Current.ControlType.ProgrammaticName + " '" + $focused.Current.Name + "'"
  if ($focused.Current.Name -eq "Increment") { "PASS  focus moved to the button" } else { "FAIL  focus is elsewhere" }
} finally {
  Stop-Process -Id $p.Id -ErrorAction SilentlyContinue
}
