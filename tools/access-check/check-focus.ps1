param([string]$Exe, [string]$Title = "Text fields")
# Moves focus with Tab and Shift+Tab, and reads where UI Automation says
# focus is. Before every key it checks that this window is the one in
# front, and stops without sending anything if it is not.
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class Fg2 {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
'@
$A = [System.Windows.Automation.AutomationElement]
$T = [System.Windows.Automation.TreeScope]

$p = Start-Process -FilePath $Exe -PassThru -NoNewWindow
try {
  $byProcess = New-Object System.Windows.Automation.PropertyCondition($A::ProcessIdProperty, $p.Id)
  $window = $null
  for ($i = 0; $i -lt 100 -and -not $window; $i++) {
    Start-Sleep -Milliseconds 100
    foreach ($w in $A::RootElement.FindAll($T::Children, $byProcess)) { if ($w.Current.Name -eq $Title) { $window = $w } }
  }
  if (-not $window) { "FAIL  no window called '$Title'"; return }
  $hwnd = [IntPtr]$window.Current.NativeWindowHandle
  Start-Sleep -Milliseconds 800
  [Fg2]::SetForegroundWindow($hwnd) | Out-Null
  Start-Sleep -Milliseconds 400

  function Send-Checked($keys) {
    if ([Fg2]::GetForegroundWindow() -ne $hwnd) { throw "STOPPED: the window under test is not in front; nothing was sent" }
    [System.Windows.Forms.SendKeys]::SendWait($keys)
    Start-Sleep -Milliseconds 400
  }
  function Focused() {
    $f = $A::FocusedElement.Current
    $f.ControlType.ProgrammaticName.Replace("ControlType.", "") + " '" + $f.Name + "'"
  }

  "at the start: " + (Focused)
  $expected = @("Edit 'Name'", "Edit 'Notes'")
  $ok = $true
  Send-Checked "{TAB}"; $now = Focused; "after Tab: $now"; if ($now -ne $expected[0]) { $ok = $false }
  Send-Checked "{TAB}"; $now = Focused; "after Tab: $now"; if ($now -ne $expected[1]) { $ok = $false }
  Send-Checked "+{TAB}"; $now = Focused; "after Shift+Tab: $now"; if ($now -ne $expected[0]) { $ok = $false }
  if ($ok) { "PASS  UI Automation focus followed Tab and Shift+Tab" } else { "FAIL  focus did not follow" }
} catch {
  $_.Exception.Message
} finally {
  Stop-Process -Id $p.Id -ErrorAction SilentlyContinue
}
