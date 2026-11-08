param(
    [Parameter(Mandatory=$true)][string]$Ref,
    [Parameter(Mandatory=$true)][string]$Port,
    [int]$CropX = 0,
    [int]$CropY = 0,
    [int]$CropW = 0,
    [int]$CropH = 0,
    [int]$Search = 3
)
Add-Type -AssemblyName System.Drawing
$bmp1 = [System.Drawing.Image]::FromFile($Ref)
$bmp2 = [System.Drawing.Image]::FromFile($Port)
if ($CropW -eq 0) { $CropW = [Math]::Min($bmp1.Width, $bmp2.Width) }
if ($CropH -eq 0) { $CropH = [Math]::Min($bmp1.Height, $bmp2.Height) }
$rect = New-Object System.Drawing.Rectangle($CropX, $CropY, $CropW, $CropH)
$clone1 = $bmp1.Clone($rect, $bmp1.PixelFormat)
$total = $clone1.Width * $clone1.Height
$best = $null
for ($dy = -$Search; $dy -le $Search; $dy++) {
    for ($dx = -$Search; $dx -le $Search; $dx++) {
        $x0 = $CropX + $dx
        $y0 = $CropY + $dy
        if ($x0 -lt 0 -or $y0 -lt 0 -or ($x0 + $CropW) -gt $bmp2.Width -or ($y0 + $CropH) -gt $bmp2.Height) { continue }
        $rect2 = New-Object System.Drawing.Rectangle($x0, $y0, $CropW, $CropH)
        $clone2 = $bmp2.Clone($rect2, $bmp2.PixelFormat)
        $diff = 0
        for ($y = 0; $y -lt $CropH; $y++) {
            for ($x = 0; $x -lt $CropW; $x++) {
                if ($clone1.GetPixel($x, $y).ToArgb() -ne $clone2.GetPixel($x, $y).ToArgb()) { $diff++ }
            }
        }
        $clone2.Dispose()
        if ($null -eq $best -or $diff -lt $best.Diff) {
            $best = [PSCustomObject]@{ Diff = $diff; DX = $dx; DY = $dy }
        }
    }
}
$pct = [Math]::Round(($best.Diff / $total) * 100, 2)
Write-Host "Best offset ($($best.DX), $($best.DY)): $pct% ($($best.Diff) / $total pixels)"
$bmp1.Dispose(); $bmp2.Dispose(); $clone1.Dispose()
