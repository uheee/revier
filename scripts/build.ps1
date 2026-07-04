param(
  [ValidateSet("win")]
  [string]$Platform = "win",

  [ValidateSet("x64", "arm64")]
  [string]$Arch = "x64",

  [switch]$Package,
  [switch]$RustTest,
  [switch]$PrepareIconsOnly
)

$ErrorActionPreference = "Stop"

function Get-RepositoryRoot {
  $scriptPath = $PSCommandPath
  if (-not $scriptPath) {
    $scriptPath = $MyInvocation.MyCommand.Path
  }

  return (Resolve-Path (Join-Path (Split-Path -Parent $scriptPath) "..")).Path
}

function Test-WhitePixel {
  param(
    [System.Drawing.Color]$Color
  )

  if ($Color.A -eq 0) {
    return $true
  }

  return $Color.R -ge 250 -and $Color.G -ge 250 -and $Color.B -ge 250
}

function Get-NonWhiteBounds {
  param(
    [System.Drawing.Bitmap]$Bitmap
  )

  $minX = $Bitmap.Width
  $minY = $Bitmap.Height
  $maxX = -1
  $maxY = -1

  for ($y = 0; $y -lt $Bitmap.Height; $y += 1) {
    for ($x = 0; $x -lt $Bitmap.Width; $x += 1) {
      if (-not (Test-WhitePixel -Color $Bitmap.GetPixel($x, $y))) {
        if ($x -lt $minX) { $minX = $x }
        if ($x -gt $maxX) { $maxX = $x }
        if ($y -lt $minY) { $minY = $y }
        if ($y -gt $maxY) { $maxY = $y }
      }
    }
  }

  if ($maxX -lt 0 -or $maxY -lt 0) {
    throw "未在 icon.png 中找到非白像素，无法生成图标。"
  }

  return [System.Drawing.Rectangle]::FromLTRB($minX, $minY, $maxX + 1, $maxY + 1)
}

function Get-PaddedSquareBounds {
  param(
    [System.Drawing.Rectangle]$Bounds,
    [int]$ImageWidth,
    [int]$ImageHeight
  )

  $contentSize = [Math]::Max($Bounds.Width, $Bounds.Height)
  $targetSize = [int][Math]::Ceiling($contentSize / 0.76)
  $targetSize = [Math]::Min($targetSize, [Math]::Min($ImageWidth, $ImageHeight))

  $centerX = $Bounds.Left + ($Bounds.Width / 2.0)
  $centerY = $Bounds.Top + ($Bounds.Height / 2.0)
  $left = [int][Math]::Floor($centerX - ($targetSize / 2.0))
  $top = [int][Math]::Floor($centerY - ($targetSize / 2.0))

  if ($left -lt 0) { $left = 0 }
  if ($top -lt 0) { $top = 0 }
  if (($left + $targetSize) -gt $ImageWidth) { $left = $ImageWidth - $targetSize }
  if (($top + $targetSize) -gt $ImageHeight) { $top = $ImageHeight - $targetSize }

  return [System.Drawing.Rectangle]::new($left, $top, $targetSize, $targetSize)
}

function New-IconPngBytes {
  param(
    [System.Drawing.Bitmap]$Source,
    [System.Drawing.Rectangle]$CropBounds,
    [int]$Size
  )

  $bitmap = [System.Drawing.Bitmap]::new($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  $stream = [System.IO.MemoryStream]::new()

  try {
    $graphics.Clear([System.Drawing.Color]::Transparent)
    $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $graphics.DrawImage($Source, [System.Drawing.Rectangle]::new(0, 0, $Size, $Size), $CropBounds, [System.Drawing.GraphicsUnit]::Pixel)
    $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
    return $stream.ToArray()
  }
  finally {
    $graphics.Dispose()
    $bitmap.Dispose()
    $stream.Dispose()
  }
}

function Write-LittleEndianUInt16 {
  param(
    [System.IO.BinaryWriter]$Writer,
    [int]$Value
  )

  $Writer.Write([uint16]$Value)
}

function Write-LittleEndianUInt32 {
  param(
    [System.IO.BinaryWriter]$Writer,
    [int]$Value
  )

  $Writer.Write([uint32]$Value)
}

function Write-IcoFile {
  param(
    [string]$OutputPath,
    [hashtable[]]$Images
  )

  $fileStream = [System.IO.File]::Create($OutputPath)
  $writer = [System.IO.BinaryWriter]::new($fileStream)

  try {
    Write-LittleEndianUInt16 -Writer $writer -Value 0
    Write-LittleEndianUInt16 -Writer $writer -Value 1
    Write-LittleEndianUInt16 -Writer $writer -Value $Images.Count

    $offset = 6 + (16 * $Images.Count)
    foreach ($image in $Images) {
      $imageSize = [int]$image["Size"]
      $imageBytes = [byte[]]$image["Bytes"]
      $directorySize = if ($imageSize -eq 256) { 0 } else { $imageSize }
      $writer.Write([byte]$directorySize)
      $writer.Write([byte]$directorySize)
      $writer.Write([byte]0)
      $writer.Write([byte]0)
      Write-LittleEndianUInt16 -Writer $writer -Value 1
      Write-LittleEndianUInt16 -Writer $writer -Value 32
      Write-LittleEndianUInt32 -Writer $writer -Value $imageBytes.Length
      Write-LittleEndianUInt32 -Writer $writer -Value $offset
      $offset += $imageBytes.Length
    }

    foreach ($image in $Images) {
      $writer.Write([byte[]]$image["Bytes"])
    }
  }
  finally {
    $writer.Dispose()
    $fileStream.Dispose()
  }
}

function Prepare-WindowsIcons {
  $repositoryRoot = Get-RepositoryRoot
  $sourcePath = Join-Path $repositoryRoot "icon.png"
  $buildDirectory = Join-Path $repositoryRoot "build"
  $outputPath = Join-Path $buildDirectory "icon.ico"

  if (-not (Test-Path -LiteralPath $sourcePath)) {
    throw "未找到源图标：$sourcePath"
  }

  New-Item -ItemType Directory -Force -Path $buildDirectory | Out-Null

  Add-Type -AssemblyName System.Drawing
  $source = [System.Drawing.Bitmap]::new($sourcePath)

  try {
    Write-Host "正在读取源图标：$sourcePath"
    $bounds = Get-NonWhiteBounds -Bitmap $source
    $cropBounds = Get-PaddedSquareBounds -Bounds $bounds -ImageWidth $source.Width -ImageHeight $source.Height
    Write-Host "已计算图标裁剪区域：$($cropBounds.Left),$($cropBounds.Top),$($cropBounds.Width)x$($cropBounds.Height)"

    $images = foreach ($size in @(16, 32, 48, 256)) {
      @{
        Size = $size
        Bytes = New-IconPngBytes -Source $source -CropBounds $cropBounds -Size $size
      }
    }

    Write-IcoFile -OutputPath $outputPath -Images $images
    Write-Host "已生成 Windows 图标：$outputPath"
  }
  finally {
    $source.Dispose()
  }
}

if ($Platform -ne "win") {
  throw "当前 PowerShell 脚本仅支持 Windows 平台图标准备。"
}

Prepare-WindowsIcons

if ($PrepareIconsOnly) {
  Write-Host "仅准备图标模式已完成。"
  exit 0
}

Write-Host "Rust 构建流程将在后续任务中补充。当前参数：Platform=$Platform，Arch=$Arch，Package=$Package，RustTest=$RustTest"
