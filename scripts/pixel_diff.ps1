param(
    [Parameter(Mandatory=$true)][string]$Ref,
    [Parameter(Mandatory=$true)][string]$Port,
    [int]$CropX = 0,
    [int]$CropY = 0,
    [int]$CropW = 0,
    [int]$CropH = 0
)
Add-Type -AssemblyName System.Drawing
$bmp1 = [System.Drawing.Image]::FromFile($Ref)
$bmp2 = [System.Drawing.Image]::FromFile($Port)
if ($CropW -eq 0) { $CropW = [Math]::Min($bmp1.Width, $bmp2.Width) }
if ($CropH -eq 0) { $CropH = [Math]::Min($bmp1.Height, $bmp2.Height) }
$rect = New-Object System.Drawing.Rectangle($CropX, $CropY, $CropW, $CropH)
$clone1 = $bmp1.Clone($rect, $bmp1.PixelFormat)
$clone2 = $bmp2.Clone($rect, $bmp2.PixelFormat)
# resize clone2 to clone1 size if needed
if ($clone1.Width -ne $clone2.Width -or $clone1.Height -ne $clone2.Height) {
    $resized = New-Object System.Drawing.Bitmap($clone1.Width, $clone1.Height)
    $g = [System.Drawing.Graphics]::FromImage($resized)
    $g.DrawImage($clone2, 0, 0, $clone1.Width, $clone1.Height)
    $g.Dispose()
    $clone2.Dispose()
    $clone2 = $resized
}
$total = $clone1.Width * $clone1.Height
$diff = 0
for ($y = 0; $y -lt $clone1.Height; $y++) {
    for ($x = 0; $x -lt $clone1.Width; $x++) {
        if ($clone1.GetPixel($x,$y).ToArgb() -ne $clone2.GetPixel($x,$y).ToArgb()) { $diff++ }
    }
}
$pct = [Math]::Round(($diff / $total) * 100, 2)
Write-Host "Mismatch: $pct% ($diff / $total pixels)"
$bmp1.Dispose(); $bmp2.Dispose(); $clone1.Dispose(); $clone2.Dispose()