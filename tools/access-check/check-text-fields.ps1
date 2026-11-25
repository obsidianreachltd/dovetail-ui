param([string]$Exe, [string]$Title = "Text fields")
# Reads and edits the text-field example through UI Automation only, as
# assistive tools do. No key presses are sent.
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
  if ($edits.Count -lt 2) { "FAIL  found $($edits.Count) text fields"; return }
  $name = $edits[0]
  "PASS  two text fields, the second multi-line: " + $edits[1].Current.ControlType.ProgrammaticName

  $name.SetFocus(); Start-Sleep -Milliseconds 400
  if ($A::FocusedElement.Current.ControlType -eq [System.Windows.Automation.ControlType]::Edit) { "PASS  focus moved to the name field" } else { "FAIL  focus is not on the name field" }

  # Setting the value, as a tool can.
  $name.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue("Ada Lovelace")
  Start-Sleep -Milliseconds 500
  if (Find-Named $window "Hello, Ada Lovelace!") { "PASS  setting the value updated the greeting" } else { "FAIL  the greeting did not change" }

  # The text and the caret, through the text pattern.
  $text = $name.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
  "document text: '" + $text.DocumentRange.GetText(-1) + "'"
  $sel = $text.GetSelection()
  "selection after setting the value: '" + $sel[0].GetText(-1) + "' (empty means a caret)"

  # Select the first word through the text pattern.
  $range = $text.DocumentRange.Clone()
  $range.MoveEndpointByRange([System.Windows.Automation.Text.TextPatternRangeEndpoint]::End, $range, [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start)
  $range.ExpandToEnclosingUnit([System.Windows.Automation.Text.TextUnit]::Word)
  $range.Select(); Start-Sleep -Milliseconds 400
  $sel = $text.GetSelection()
  $picked = $sel[0].GetText(-1)
  if ($picked.Trim() -eq "Ada") { "PASS  selecting the first word through UI Automation selects '$picked'" } else { "FAIL  the selection is '$picked'" }
} finally {
  Stop-Process -Id $p.Id -ErrorAction SilentlyContinue
}
