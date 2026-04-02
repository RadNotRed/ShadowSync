param(
    [Parameter(Mandatory = $true)]
    [string]$ConfigPath
)

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

[System.Windows.Forms.Application]::EnableVisualStyles()

function Show-ErrorDialog {
    param([string]$Message)
    [System.Windows.Forms.MessageBox]::Show($Message, "USB Mirror Sync Setup", [System.Windows.Forms.MessageBoxButtons]::OK, [System.Windows.Forms.MessageBoxIcon]::Warning) | Out-Null
}

function New-Label {
    param([string]$Text, [int]$X, [int]$Y, [int]$Width = 150)
    $label = New-Object System.Windows.Forms.Label
    $label.Text = $Text
    $label.Location = New-Object System.Drawing.Point($X, $Y)
    $label.Size = New-Object System.Drawing.Size($Width, 22)
    return $label
}

function New-TextBox {
    param([int]$X, [int]$Y, [int]$Width = 180)
    $textBox = New-Object System.Windows.Forms.TextBox
    $textBox.Location = New-Object System.Drawing.Point($X, $Y)
    $textBox.Size = New-Object System.Drawing.Size($Width, 24)
    return $textBox
}

function Convert-UsbPathToRelative {
    param([string]$DriveLetter, [string]$SelectedPath)
    $prefix = ($DriveLetter.Trim().TrimEnd(':') + ':\')
    if ($SelectedPath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $SelectedPath.Substring($prefix.Length).TrimStart('\')
    }
    return $null
}

$form = New-Object System.Windows.Forms.Form
$form.Text = "USB Mirror Sync Setup"
$form.StartPosition = "CenterScreen"
$form.Size = New-Object System.Drawing.Size(980, 690)
$form.MinimumSize = New-Object System.Drawing.Size(980, 690)
$form.Font = New-Object System.Drawing.Font("Segoe UI", 9)

$note = New-Object System.Windows.Forms.Label
$note.Location = New-Object System.Drawing.Point(16, 12)
$note.Size = New-Object System.Drawing.Size(930, 44)
$note.Text = "USB is the source of truth. Shadow is the local USB cache, and the target folder on your PC is the live mirrored copy."
$note.ForeColor = [System.Drawing.Color]::FromArgb(60, 60, 60)
$form.Controls.Add($note)

$form.Controls.Add((New-Label "USB Drive Letter" 16 66 120))
$txtDrive = New-TextBox 140 62 50
$form.Controls.Add($txtDrive)

$form.Controls.Add((New-Label "Poll Seconds" 220 66 100))
$numPoll = New-Object System.Windows.Forms.NumericUpDown
$numPoll.Location = New-Object System.Drawing.Point(320, 62)
$numPoll.Size = New-Object System.Drawing.Size(70, 24)
$numPoll.Minimum = 1
$numPoll.Maximum = 60
$numPoll.Value = 2
$form.Controls.Add($numPoll)

$chkSyncOnInsert = New-Object System.Windows.Forms.CheckBox
$chkSyncOnInsert.Text = "Sync on insert"
$chkSyncOnInsert.Location = New-Object System.Drawing.Point(430, 64)
$chkSyncOnInsert.Size = New-Object System.Drawing.Size(120, 24)
$chkSyncOnInsert.Checked = $true
$form.Controls.Add($chkSyncOnInsert)

$chkSyncWhileMounted = New-Object System.Windows.Forms.CheckBox
$chkSyncWhileMounted.Text = "Watch USB while mounted"
$chkSyncWhileMounted.Location = New-Object System.Drawing.Point(560, 64)
$chkSyncWhileMounted.Size = New-Object System.Drawing.Size(180, 24)
$chkSyncWhileMounted.Checked = $true
$form.Controls.Add($chkSyncWhileMounted)

$chkAutoSyncToUsb = New-Object System.Windows.Forms.CheckBox
$chkAutoSyncToUsb.Text = "Auto sync target back to USB"
$chkAutoSyncToUsb.Location = New-Object System.Drawing.Point(740, 64)
$chkAutoSyncToUsb.Size = New-Object System.Drawing.Size(220, 24)
$chkAutoSyncToUsb.Checked = $false
$form.Controls.Add($chkAutoSyncToUsb)

$chkEject = New-Object System.Windows.Forms.CheckBox
$chkEject.Text = "Eject after sync"
$chkEject.Location = New-Object System.Drawing.Point(16, 98)
$chkEject.Size = New-Object System.Drawing.Size(130, 24)
$form.Controls.Add($chkEject)

$chkHash = New-Object System.Windows.Forms.CheckBox
$chkHash.Text = "Hash when metadata changes"
$chkHash.Location = New-Object System.Drawing.Point(170, 98)
$chkHash.Size = New-Object System.Drawing.Size(190, 24)
$chkHash.Checked = $true
$form.Controls.Add($chkHash)

$chkShadow = New-Object System.Windows.Forms.CheckBox
$chkShadow.Text = "Use shadow cache"
$chkShadow.Location = New-Object System.Drawing.Point(400, 98)
$chkShadow.Size = New-Object System.Drawing.Size(150, 24)
$chkShadow.Checked = $true
$form.Controls.Add($chkShadow)

$chkClearShadow = New-Object System.Windows.Forms.CheckBox
$chkClearShadow.Text = "Clear shadow cache on eject"
$chkClearShadow.Location = New-Object System.Drawing.Point(560, 98)
$chkClearShadow.Size = New-Object System.Drawing.Size(220, 24)
$chkClearShadow.Checked = $false
$form.Controls.Add($chkClearShadow)

$grid = New-Object System.Windows.Forms.DataGridView
$grid.Location = New-Object System.Drawing.Point(16, 138)
$grid.Size = New-Object System.Drawing.Size(932, 430)
$grid.AllowUserToAddRows = $false
$grid.AllowUserToDeleteRows = $false
$grid.MultiSelect = $false
$grid.SelectionMode = "FullRowSelect"
$grid.AutoSizeColumnsMode = "Fill"
$grid.RowHeadersVisible = $false
$grid.EditMode = "EditOnEnter"
$grid.Columns.Add("Name", "Name") | Out-Null
$grid.Columns.Add("Source", "USB Source (relative)") | Out-Null
$grid.Columns.Add("Target", "Local Target (absolute)") | Out-Null
$mirrorColumn = New-Object System.Windows.Forms.DataGridViewCheckBoxColumn
$mirrorColumn.Name = "MirrorDeletes"
$mirrorColumn.HeaderText = "Mirror Deletes"
$grid.Columns.Add($mirrorColumn) | Out-Null
$grid.Columns["Name"].FillWeight = 18
$grid.Columns["Source"].FillWeight = 27
$grid.Columns["Target"].FillWeight = 40
$grid.Columns["MirrorDeletes"].FillWeight = 15
$form.Controls.Add($grid)

$btnAdd = New-Object System.Windows.Forms.Button
$btnAdd.Text = "Add Job"
$btnAdd.Location = New-Object System.Drawing.Point(16, 582)
$btnAdd.Size = New-Object System.Drawing.Size(100, 32)
$form.Controls.Add($btnAdd)

$btnRemove = New-Object System.Windows.Forms.Button
$btnRemove.Text = "Remove Job"
$btnRemove.Location = New-Object System.Drawing.Point(126, 582)
$btnRemove.Size = New-Object System.Drawing.Size(110, 32)
$form.Controls.Add($btnRemove)

$btnBrowseUsb = New-Object System.Windows.Forms.Button
$btnBrowseUsb.Text = "Browse USB Source"
$btnBrowseUsb.Location = New-Object System.Drawing.Point(256, 582)
$btnBrowseUsb.Size = New-Object System.Drawing.Size(140, 32)
$form.Controls.Add($btnBrowseUsb)

$btnBrowseTarget = New-Object System.Windows.Forms.Button
$btnBrowseTarget.Text = "Browse Local Target"
$btnBrowseTarget.Location = New-Object System.Drawing.Point(406, 582)
$btnBrowseTarget.Size = New-Object System.Drawing.Size(145, 32)
$form.Controls.Add($btnBrowseTarget)

$btnOpenRaw = New-Object System.Windows.Forms.Button
$btnOpenRaw.Text = "Open Raw Config"
$btnOpenRaw.Location = New-Object System.Drawing.Point(571, 582)
$btnOpenRaw.Size = New-Object System.Drawing.Size(125, 32)
$form.Controls.Add($btnOpenRaw)

$btnSave = New-Object System.Windows.Forms.Button
$btnSave.Text = "Save"
$btnSave.Location = New-Object System.Drawing.Point(742, 582)
$btnSave.Size = New-Object System.Drawing.Size(90, 32)
$form.Controls.Add($btnSave)

$btnCancel = New-Object System.Windows.Forms.Button
$btnCancel.Text = "Cancel"
$btnCancel.Location = New-Object System.Drawing.Point(842, 582)
$btnCancel.Size = New-Object System.Drawing.Size(90, 32)
$form.Controls.Add($btnCancel)

$form.AcceptButton = $btnSave
$form.CancelButton = $btnCancel

if (Test-Path -LiteralPath $ConfigPath) {
    try {
        $config = Get-Content -LiteralPath $ConfigPath -Raw | ConvertFrom-Json
        if ($null -ne $config.drive) {
            $txtDrive.Text = [string]$config.drive.letter
            $chkEject.Checked = [bool]$config.drive.eject_after_sync
        }
        if ($null -ne $config.app) {
            if ($null -ne $config.app.poll_interval_seconds) { $numPoll.Value = [decimal]$config.app.poll_interval_seconds }
            $chkSyncOnInsert.Checked = [bool]$config.app.sync_on_insert
            if ($null -ne $config.app.sync_while_mounted) { $chkSyncWhileMounted.Checked = [bool]$config.app.sync_while_mounted }
            if ($null -ne $config.app.auto_sync_to_usb) { $chkAutoSyncToUsb.Checked = [bool]$config.app.auto_sync_to_usb }
        }
        if ($null -ne $config.cache) {
            $chkShadow.Checked = [bool]$config.cache.shadow_copy
            $chkClearShadow.Checked = [bool]$config.cache.clear_shadow_on_eject
        }
        if ($null -ne $config.compare) {
            $chkHash.Checked = [bool]$config.compare.hash_on_metadata_change
        }
        if ($null -ne $config.jobs) {
            foreach ($job in $config.jobs) {
                $grid.Rows.Add([string]$job.name, [string]$job.source, [string]$job.target, [bool]$job.mirror_deletes) | Out-Null
            }
        }
    } catch {
        Show-ErrorDialog "Failed to load existing config.`n`n$($_.Exception.Message)"
    }
}

if ($grid.Rows.Count -eq 0) {
    $grid.Rows.Add("Documents", "Backups\Documents", "C:\Users\$env:USERNAME\Documents\Important", $true) | Out-Null
}

$btnAdd.Add_Click({
    $grid.Rows.Add("", "", "", $true) | Out-Null
    $grid.CurrentCell = $grid.Rows[$grid.Rows.Count - 1].Cells["Name"]
    $grid.BeginEdit($true) | Out-Null
})

$btnRemove.Add_Click({
    if ($grid.SelectedRows.Count -gt 0) {
        $grid.Rows.RemoveAt($grid.SelectedRows[0].Index)
    }
})

$btnBrowseUsb.Add_Click({
    if ($grid.SelectedRows.Count -eq 0) {
        Show-ErrorDialog "Select a job row first."
        return
    }

    $driveLetter = $txtDrive.Text.Trim().TrimEnd(':')
    if ([string]::IsNullOrWhiteSpace($driveLetter)) {
        Show-ErrorDialog "Enter the USB drive letter first."
        return
    }

    $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
    $dialog.Description = "Choose a folder on the USB drive."
    $dialog.UseDescriptionForTitle = $true
    if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
        $relative = Convert-UsbPathToRelative -DriveLetter $driveLetter -SelectedPath $dialog.SelectedPath
        if ($null -eq $relative) {
            Show-ErrorDialog "Selected folder is not inside drive $driveLetter`:"
            return
        }
        $grid.SelectedRows[0].Cells["Source"].Value = $relative
    }
})

$btnBrowseTarget.Add_Click({
    if ($grid.SelectedRows.Count -eq 0) {
        Show-ErrorDialog "Select a job row first."
        return
    }

    $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
    $dialog.Description = "Choose the local target folder."
    $dialog.UseDescriptionForTitle = $true
    if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
        $grid.SelectedRows[0].Cells["Target"].Value = $dialog.SelectedPath
    }
})

$btnOpenRaw.Add_Click({
    Start-Process notepad.exe -ArgumentList @($ConfigPath) | Out-Null
})

$btnCancel.Add_Click({
    $form.Close()
})

$btnSave.Add_Click({
    try {
        $driveLetter = $txtDrive.Text.Trim().TrimEnd(':')
        if ($driveLetter.Length -ne 1 -or -not [char]::IsLetter($driveLetter[0])) {
            throw "Drive letter must be a single letter like S."
        }

        $jobs = @()
        foreach ($row in $grid.Rows) {
            if ($row.IsNewRow) { continue }

            $name = [string]$row.Cells["Name"].Value
            $source = [string]$row.Cells["Source"].Value
            $target = [string]$row.Cells["Target"].Value
            $mirrorDeletes = [bool]$row.Cells["MirrorDeletes"].Value

            if ([string]::IsNullOrWhiteSpace($name) -and [string]::IsNullOrWhiteSpace($source) -and [string]::IsNullOrWhiteSpace($target)) {
                continue
            }

            if ([string]::IsNullOrWhiteSpace($name)) { throw "Every job needs a name." }
            if ([string]::IsNullOrWhiteSpace($source)) { throw "Every job needs a USB source path." }
            if ([string]::IsNullOrWhiteSpace($target)) { throw "Every job needs a local target path." }
            if ([System.IO.Path]::IsPathRooted($source)) { throw "USB source paths must be relative. Do not include $driveLetter`:\" }
            if (-not [System.IO.Path]::IsPathRooted($target)) { throw "Local target paths must be absolute." }

            $jobs += [pscustomobject]@{
                name = $name
                source = $source
                target = $target
                mirror_deletes = $mirrorDeletes
            }
        }

        if ($jobs.Count -eq 0) {
            throw "Add at least one job."
        }

        $config = [pscustomobject]@{
            drive = [pscustomobject]@{
                letter = $driveLetter.ToUpperInvariant()
                eject_after_sync = [bool]$chkEject.Checked
            }
            app = [pscustomobject]@{
                sync_on_insert = [bool]$chkSyncOnInsert.Checked
                sync_while_mounted = [bool]$chkSyncWhileMounted.Checked
                auto_sync_to_usb = [bool]$chkAutoSyncToUsb.Checked
                poll_interval_seconds = [int]$numPoll.Value
            }
            cache = [pscustomobject]@{
                shadow_copy = [bool]$chkShadow.Checked
                clear_shadow_on_eject = [bool]$chkClearShadow.Checked
            }
            compare = [pscustomobject]@{
                hash_on_metadata_change = [bool]$chkHash.Checked
            }
            jobs = $jobs
        }

        $directory = Split-Path -Parent $ConfigPath
        if (-not (Test-Path -LiteralPath $directory)) {
            New-Item -ItemType Directory -Path $directory -Force | Out-Null
        }

        $json = $config | ConvertTo-Json -Depth 6
        $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
        [System.IO.File]::WriteAllText($ConfigPath, $json, $utf8NoBom)
        [System.Windows.Forms.MessageBox]::Show("Config saved.", "USB Mirror Sync Setup", [System.Windows.Forms.MessageBoxButtons]::OK, [System.Windows.Forms.MessageBoxIcon]::Information) | Out-Null
        $form.Close()
    } catch {
        Show-ErrorDialog $_.Exception.Message
    }
})

[void]$form.ShowDialog()
