# Makes the test images in this folder with Windows' own encoders (GDI+), so
# Dovetail's decoders are checked against files they did not write (M17).
# Run it from this folder in Windows PowerShell. Each pixel of the source is
# (4x, 5y, 2(x + y)), the formula tests/decode.rs checks against.
Add-Type -AssemblyName System.Drawing
$w = 64; $h = 48
$bmp = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
for ($y = 0; $y -lt $h; $y++) {
  for ($x = 0; $x -lt $w; $x++) {
    $bmp.SetPixel($x, $y, [System.Drawing.Color]::FromArgb(4 * $x, 5 * $y, 2 * ($x + $y)))
  }
}
$here = (Get-Location).Path
$bmp.Save("$here\gdi.png", [System.Drawing.Imaging.ImageFormat]::Png)
$codec = [System.Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object { $_.MimeType -eq 'image/jpeg' }
$params = New-Object System.Drawing.Imaging.EncoderParameters 1
$params.Param[0] = New-Object System.Drawing.Imaging.EncoderParameter ([System.Drawing.Imaging.Encoder]::Quality), ([long]90)
$bmp.Save("$here\gdi.jpg", $codec, $params)
$bmp.Dispose()
# What GDI+ itself decodes the JPEG file to, as red, green, blue bytes.
$back = New-Object System.Drawing.Bitmap "$here\gdi.jpg"
$out = New-Object byte[] ($w * $h * 3)
for ($y = 0; $y -lt $h; $y++) {
  for ($x = 0; $x -lt $w; $x++) {
    $c = $back.GetPixel($x, $y)
    $i = ($y * $w + $x) * 3
    $out[$i] = $c.R; $out[$i + 1] = $c.G; $out[$i + 2] = $c.B
  }
}
$back.Dispose()
[System.IO.File]::WriteAllBytes("$here\gdi-jpg.rgb", $out)
