param([string]$Exe, [string]$Title = "Commands", [string]$Shots = "")
# Reads the commands example's menu bar, menus and toolbar through UI
# Automation, opens menus and runs items by invoking them, and checks that
# disabled commands are disabled everywhere. No key presses are sent. With
# -Shots, saves pictures of the open menu and of command search there.
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class Fg3 { [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); }
'@
$A = [System.Windows.Automation.AutomationElement]
$T = [System.Windows.Automation.TreeScope]
$All = [System.Windows.Automation.Condition]::TrueCondition

function Find-All($root, $name) {
  @($root.FindAll($T::Descendants, $All) | Where-Object { $_.Current.Name -eq $name })
}
function Find-One($root, $name, $type) {
  foreach ($e in Find-All $root $name) {
    if (-not $type -or $e.Current.ControlType.ProgrammaticName -eq "ControlType.$type") { return $e }
  }
  return $null
}
function Invoke-Element($e) {
  $e.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
  Start-Sleep -Milliseconds 500
}
function Show-Menu($window, $menuName) {
  $menu = Find-One $window $menuName "Menu"
  if (-not $menu) { return $null }
  foreach ($item in $menu.FindAll($T::Children, $All)) {
    "    " + $item.Current.ControlType.ProgrammaticName.Replace("ControlType.", "") + " '" + $item.Current.Name + "' enabled=" + $item.Current.IsEnabled
  }
}
function Save-Shot($window, $file) {
  if (-not $Shots) { return }
  [Fg3]::SetForegroundWindow([IntPtr]$window.Current.NativeWindowHandle) | Out-Null
  Start-Sleep -Milliseconds 400
  $r = $window.Current.BoundingRectangle
  $bmp = New-Object System.Drawing.Bitmap ([int]$r.Width), ([int]$r.Height)
  [System.Drawing.Graphics]::FromImage($bmp).CopyFromScreen([int]$r.X, [int]$r.Y, 0, 0, $bmp.Size)
  $bmp.Save((Join-Path $Shots $file))
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
  $bar = $null
  for ($i = 0; $i -lt 30 -and -not $bar; $i++) { Start-Sleep -Milliseconds 100; $bar = Find-One $window "Menu bar" "MenuBar" }
  if (-not $bar) { "FAIL  no menu bar"; return }
  "--- the bar and toolbar"
  foreach ($e in $bar.FindAll($T::Children, $All)) { "  menu title '" + $e.Current.Name + "'" }
  $toolbar = Find-One $window "Toolbar" "ToolBar"
  foreach ($e in $toolbar.FindAll($T::Children, $All)) {
    "  toolbar " + $e.Current.ControlType.ProgrammaticName.Replace("ControlType.", "") + " '" + $e.Current.Name + "' enabled=" + $e.Current.IsEnabled
  }

  "--- the List menu, with an empty list"
  Invoke-Element (Find-One $bar "List" "MenuItem")
  Show-Menu $window "List"
  Save-Shot $window "menu.png"
  $remove = Find-One (Find-One $window "List" "Menu") "Remove last item" "MenuItem"
  $removeButton = Find-One $toolbar "Remove last item" "Button"
  if (-not $remove.Current.IsEnabled -and -not $removeButton.Current.IsEnabled) {
    "PASS  with an empty list, Remove last item is disabled in the menu and the toolbar"
  } else { "FAIL  Remove last item is enabled with an empty list" }

  Invoke-Element (Find-One (Find-One $window "List" "Menu") "Add item" "MenuItem")
  if (Find-One $window "Items: 1") { "PASS  choosing Add item from the menu added an item" } else { "FAIL  the menu item did nothing" }
  $removeButton = Find-One (Find-One $window "Toolbar" "ToolBar") "Remove last item" "Button"
  if ($removeButton.Current.IsEnabled) { "PASS  with an item, Remove last item is enabled again" } else { "FAIL  still disabled" }

  "--- the Edit menu"
  Invoke-Element (Find-One $bar "Edit" "MenuItem")
  Show-Menu $window "Edit"
  Invoke-Element (Find-One (Find-One $window "Edit" "Menu") "Search commands" "MenuItem")
  $search = Find-One $window "Search commands" "Edit"
  if ($search) { "PASS  command search opened from the menu, with its search box" } else { "FAIL  command search did not open" }
  Save-Shot $window "search.png"
} finally {
  Stop-Process -Id $p.Id -ErrorAction SilentlyContinue
}
