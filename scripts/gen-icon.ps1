Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$assets = Join-Path $root "assets"
New-Item -ItemType Directory -Force -Path $assets | Out-Null

function New-NeoChatBitmap([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.Clear([System.Drawing.Color]::Transparent)

    $pad = [single]($size * 0.06)
    $side = [single]($size - 2 * $pad)
    $radius = [single]($side * 0.24)
    $rect = New-Object System.Drawing.RectangleF($pad, $pad, $side, $side)

    $shape = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $radius * 2
    $shape.AddArc($rect.X, $rect.Y, $d, $d, 180, 90)
    $shape.AddArc($rect.Right - $d, $rect.Y, $d, $d, 270, 90)
    $shape.AddArc($rect.Right - $d, $rect.Bottom - $d, $d, $d, 0, 90)
    $shape.AddArc($rect.X, $rect.Bottom - $d, $d, $d, 90, 90)
    $shape.CloseFigure()

    $start = [System.Drawing.Color]::FromArgb(255, 0, 96, 184)
    $end = [System.Drawing.Color]::FromArgb(255, 0, 140, 220)
    $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush($rect, $start, $end, 55.0)
    $g.FillPath($brush, $shape)
    $brush.Dispose()

    $pen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(70, 255, 255, 255), [single]([Math]::Max(1, $size * 0.008)))
    $g.DrawPath($pen, $shape)
    $pen.Dispose()

    $bubbleW = [single]($side * 0.56)
    $bubbleH = [single]($side * 0.42)
    $bubbleX = [single]($pad + ($side - $bubbleW) / 2)
    $bubbleY = [single]($pad + ($side - $bubbleH) / 2 - $side * 0.03)
    $bubbleR = [single]($bubbleH * 0.42)
    $bubble = New-Object System.Drawing.RectangleF($bubbleX, $bubbleY, $bubbleW, $bubbleH)

    $bubblePath = New-Object System.Drawing.Drawing2D.GraphicsPath
    $bd = $bubbleR * 2
    $bubblePath.AddArc($bubble.X, $bubble.Y, $bd, $bd, 180, 90)
    $bubblePath.AddArc($bubble.Right - $bd, $bubble.Y, $bd, $bd, 270, 90)
    $bubblePath.AddArc($bubble.Right - $bd, $bubble.Bottom - $bd, $bd, $bd, 0, 90)
    $bubblePath.AddArc($bubble.X, $bubble.Bottom - $bd, $bd, $bd, 90, 90)
    $bubblePath.CloseFigure()

    $tail = New-Object System.Drawing.Drawing2D.GraphicsPath
    $tx = $bubbleX + $bubbleW * 0.28
    $ty = $bubbleY + $bubbleH * 0.82
    $tail.AddPolygon(@(
        (New-Object System.Drawing.PointF($tx, $ty)),
        (New-Object System.Drawing.PointF(($tx + $bubbleW * 0.18), $ty)),
        (New-Object System.Drawing.PointF($tx, ($ty + $bubbleH * 0.34)))
    ))

    $white = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
    $g.FillPath($white, $bubblePath)
    $g.FillPath($white, $tail)
    $white.Dispose()
    $bubblePath.Dispose()
    $tail.Dispose()

    $dotR = [single]($bubbleH * 0.10)
    $dotY = [single]($bubbleY + $bubbleH * 0.5)
    $gap = [single]($bubbleW * 0.20)
    $centerX = [single]($bubbleX + $bubbleW * 0.5)
    $dot = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 0, 110, 200))
    foreach ($offset in -1, 0, 1) {
        $cx = [single]($centerX + $offset * $gap)
        $g.FillEllipse($dot, [single]($cx - $dotR), [single]($dotY - $dotR), [single]($dotR * 2), [single]($dotR * 2))
    }
    $dot.Dispose()

    $shape.Dispose()
    $g.Dispose()
    return $bmp
}

function Get-IconEntryBytes([System.Drawing.Bitmap]$bmp) {
    $w = $bmp.Width
    $h = $bmp.Height
    $rect = New-Object System.Drawing.Rectangle(0, 0, $w, $h)
    $data = $bmp.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $stride = $data.Stride
    $pixels = New-Object byte[] ($stride * $h)
    [System.Runtime.InteropServices.Marshal]::Copy($data.Scan0, $pixels, 0, $pixels.Length)
    $bmp.UnlockBits($data)

    $maskStride = [int]([Math]::Floor(($w + 31) / 32) * 4)
    $mask = New-Object byte[] ($maskStride * $h)

    $ms = New-Object System.IO.MemoryStream
    $bw = New-Object System.IO.BinaryWriter($ms)
    $bw.Write([uint32]40)
    $bw.Write([int32]$w)
    $bw.Write([int32]($h * 2))
    $bw.Write([uint16]1)
    $bw.Write([uint16]32)
    $bw.Write([uint32]0)
    $bw.Write([uint32]($w * $h * 4 + $mask.Length))
    $bw.Write([int32]0)
    $bw.Write([int32]0)
    $bw.Write([uint32]0)
    $bw.Write([uint32]0)

    for ($y = $h - 1; $y -ge 0; $y--) {
        for ($x = 0; $x -lt $w; $x++) {
            $i = $y * $stride + $x * 4
            $b = $pixels[$i]
            $gch = $pixels[$i + 1]
            $r = $pixels[$i + 2]
            $a = $pixels[$i + 3]
            $bw.Write([byte]$b)
            $bw.Write([byte]$gch)
            $bw.Write([byte]$r)
            $bw.Write([byte]$a)
            if ($a -eq 0) {
                $maskY = $h - 1 - $y
                $maskIndex = $maskY * $maskStride + [int]([Math]::Floor($x / 8))
                $mask[$maskIndex] = $mask[$maskIndex] -bor (1 -shl (7 - ($x % 8)))
            }
        }
    }
    $bw.Write($mask)
    $bw.Flush()
    $bytes = $ms.ToArray()
    $bw.Dispose()
    $ms.Dispose()
    return $bytes
}

$sizes = 16, 20, 24, 32, 40, 48, 64, 128, 256
$entries = @()
foreach ($s in $sizes) {
    $bmp = New-NeoChatBitmap $s
    $bytes = [byte[]](Get-IconEntryBytes $bmp)
    $entries += [pscustomobject]@{ Size = $s; Bytes = $bytes }
    if ($s -eq 256) {
        $bmp.Save((Join-Path $assets "neochat.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    }
    $bmp.Dispose()
}

$icoPath = Join-Path $assets "neochat.ico"
$fs = [System.IO.File]::Create($icoPath)
$bw = New-Object System.IO.BinaryWriter($fs)
$bw.Write([uint16]0)
$bw.Write([uint16]1)
$bw.Write([uint16]$entries.Count)
$offset = 6 + 16 * $entries.Count
foreach ($e in $entries) {
    $dim = if ($e.Size -ge 256) { 0 } else { $e.Size }
    $bw.Write([byte]$dim)
    $bw.Write([byte]$dim)
    $bw.Write([byte]0)
    $bw.Write([byte]0)
    $bw.Write([uint16]1)
    $bw.Write([uint16]32)
    $bw.Write([uint32]$e.Bytes.Length)
    $bw.Write([uint32]$offset)
    $offset += $e.Bytes.Length
}
foreach ($e in $entries) {
    $bw.Write($e.Bytes)
}
$bw.Flush()
$bw.Dispose()
$fs.Dispose()

Write-Output "Gerado: $icoPath"
Write-Output "Gerado: $(Join-Path $assets 'neochat.png')"
