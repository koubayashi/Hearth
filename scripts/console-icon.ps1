# Renders the rail's Console glyph (Icon.tsx "gamepad": 24px grid, 1.8 stroke,
# round joins) to the two .ico files the taskbar jump list uses: light strokes
# for a dark taskbar, dark for a light one. WPF reads SVG path data as is.
# Run: powershell -STA -File scripts/console-icon.ps1
Add-Type -AssemblyName PresentationCore, WindowsBase
$paths = @(
  "M7.5 7.5h9a4.5 4.5 0 0 1 4.4 5.4l-.7 3.4a2.4 2.4 0 0 1-4.2 1L14.6 15H9.4L8 17.3a2.4 2.4 0 0 1-4.2-1l-.7-3.4a4.5 4.5 0 0 1 4.4-5.4z",
  "M8.5 10.5v3M7 12h3M15.5 11h.01M17 13h.01"
)
$sizes = 16, 20, 24, 32, 40, 48, 64, 256

function Render-Png([int]$size, [string]$color) {
  $brush = New-Object System.Windows.Media.SolidColorBrush ([System.Windows.Media.ColorConverter]::ConvertFromString($color))
  $pen = New-Object System.Windows.Media.Pen $brush, 1.8
  $pen.StartLineCap = "Round"; $pen.EndLineCap = "Round"; $pen.LineJoin = "Round"
  $visual = New-Object System.Windows.Media.DrawingVisual
  $dc = $visual.RenderOpen()
  $dc.PushTransform((New-Object System.Windows.Media.ScaleTransform ($size / 24), ($size / 24)))
  foreach ($d in $paths) { $dc.DrawGeometry($null, $pen, [System.Windows.Media.Geometry]::Parse($d)) }
  $dc.Pop(); $dc.Close()
  $bmp = New-Object System.Windows.Media.Imaging.RenderTargetBitmap $size, $size, 96, 96, ([System.Windows.Media.PixelFormats]::Pbgra32)
  $bmp.Render($visual)
  $enc = New-Object System.Windows.Media.Imaging.PngBitmapEncoder
  $enc.Frames.Add([System.Windows.Media.Imaging.BitmapFrame]::Create($bmp))
  $ms = New-Object System.IO.MemoryStream
  $enc.Save($ms)
  return ,$ms.ToArray()
}

# An .ico is a directory of entries; each entry may simply hold a PNG.
function Write-Ico([string]$file, [string]$color) {
  $imgs = @($sizes | ForEach-Object { ,(Render-Png $_ $color) })
  $out = New-Object System.IO.MemoryStream
  $w = New-Object System.IO.BinaryWriter $out
  $w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$imgs.Count)
  $offset = 6 + 16 * $imgs.Count
  for ($i = 0; $i -lt $imgs.Count; $i++) {
    $s = $sizes[$i]; $b = if ($s -ge 256) { 0 } else { $s }
    $w.Write([byte]$b); $w.Write([byte]$b); $w.Write([byte]0); $w.Write([byte]0)
    $w.Write([uint16]1); $w.Write([uint16]32)
    $w.Write([uint32]$imgs[$i].Length); $w.Write([uint32]$offset)
    $offset += $imgs[$i].Length
  }
  foreach ($img in $imgs) { $w.Write($img) }
  $w.Flush()
  [System.IO.File]::WriteAllBytes($file, $out.ToArray())
}

$dir = Join-Path $PSScriptRoot "..\app\src-tauri\icons"
Write-Ico (Join-Path $dir "console-light.ico") "#FFFFFF"
Write-Ico (Join-Path $dir "console-dark.ico") "#1A1A1A"
"wrote console-light.ico and console-dark.ico"
