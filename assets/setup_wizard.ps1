param(
    [Parameter(Mandatory = $true)]
    [string]$ConfigPath,
    [string]$ErrorMessage = "",
    [string]$RecoveryBackupPath = "",
    [switch]$RecoveredDefault
)

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

[System.Windows.Forms.Application]::EnableVisualStyles()

function Show-ErrorDialog {
    param([string]$Message)
    [System.Windows.Forms.MessageBox]::Show(
        $Message,
        "USB Mirror Sync Setup",
        [System.Windows.Forms.MessageBoxButtons]::OK,
        [System.Windows.Forms.MessageBoxIcon]::Warning
    ) | Out-Null
}

function New-Label {
    param(
        [string]$Text,
        [int]$X,
        [int]$Y,
        [int]$Width = 200,
        [int]$Height = 22,
        [bool]$Bold = $false
    )

    $label = New-Object System.Windows.Forms.Label
    $label.Text = $Text
    $label.Location = New-Object System.Drawing.Point($X, $Y)
    $label.Size = New-Object System.Drawing.Size($Width, $Height)
    if ($Bold) {
        $label.Font = New-Object System.Drawing.Font("Segoe UI Semibold", 9)
    }
    return $label
}

function New-TextBox {
    param(
        [int]$X,
        [int]$Y,
        [int]$Width = 220,
        [string]$Text = ""
    )

    $textBox = New-Object System.Windows.Forms.TextBox
    $textBox.Location = New-Object System.Drawing.Point($X, $Y)
    $textBox.Size = New-Object System.Drawing.Size($Width, 24)
    $textBox.Text = $Text
    return $textBox
}

function New-InfoLabel {
    param(
        [string]$Text,
        [int]$X,
        [int]$Y,
        [int]$Width = 440,
        [int]$Height = 40
    )

    $label = New-Object System.Windows.Forms.Label
    $label.Text = $Text
    $label.Location = New-Object System.Drawing.Point($X, $Y)
    $label.Size = New-Object System.Drawing.Size($Width, $Height)
    $label.ForeColor = [System.Drawing.Color]::FromArgb(84, 92, 104)
    return $label
}

function Convert-UsbPathToRelative {
    param([string]$DriveLetter, [string]$SelectedPath)
    $prefix = ($DriveLetter.Trim().TrimEnd(':') + ':\')
    if ($SelectedPath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $SelectedPath.Substring($prefix.Length).TrimStart('\')
    }
    return $null
}

function Set-Status {
    param([string]$Message)
    $lblStatus.Text = $Message
}

function Get-SelectedJobRow {
    if ($grid.SelectedRows.Count -gt 0) {
        return $grid.SelectedRows[0]
    }
    if ($grid.CurrentRow -ne $null) {
        return $grid.CurrentRow
    }
    return $null
}

function Apply-DefaultValues {
    $txtDrive.Text = "E"
    $numPoll.Value = 2
    $chkSyncOnInsert.Checked = $true
    $chkSyncWhileMounted.Checked = $true
    $chkAutoSyncToUsb.Checked = $false
    $chkEject.Checked = $false
    $chkHash.Checked = $true
    $chkShadow.Checked = $true
    $chkClearShadow.Checked = $false
    $grid.Rows.Clear()
    $grid.Rows.Add("Documents", "Backups\Documents", "C:\Users\$env:USERNAME\Documents\Important", $true) | Out-Null
}

$form = New-Object System.Windows.Forms.Form
$form.Text = "USB Mirror Sync Setup"
$form.StartPosition = "CenterScreen"
$form.Size = New-Object System.Drawing.Size(1140, 820)
$form.MinimumSize = New-Object System.Drawing.Size(1140, 820)
$form.BackColor = [System.Drawing.Color]::FromArgb(245, 247, 250)
$form.Font = New-Object System.Drawing.Font("Segoe UI", 9)

$toolTip = New-Object System.Windows.Forms.ToolTip
$toolTip.AutoPopDelay = 14000
$toolTip.InitialDelay = 250
$toolTip.ReshowDelay = 150
$toolTip.ShowAlways = $true

$headerPanel = New-Object System.Windows.Forms.Panel
$headerPanel.Location = New-Object System.Drawing.Point(18, 16)
$headerPanel.Size = New-Object System.Drawing.Size(1088, 86)
$headerPanel.BackColor = [System.Drawing.Color]::FromArgb(28, 74, 120)
$form.Controls.Add($headerPanel)

$headerTitle = New-Object System.Windows.Forms.Label
$headerTitle.Text = "USB Mirror Sync Setup"
$headerTitle.Location = New-Object System.Drawing.Point(24, 16)
$headerTitle.Size = New-Object System.Drawing.Size(500, 28)
$headerTitle.ForeColor = [System.Drawing.Color]::White
$headerTitle.Font = New-Object System.Drawing.Font("Segoe UI Semibold", 15)
$headerPanel.Controls.Add($headerTitle)

$headerSubtitle = New-Object System.Windows.Forms.Label
$headerSubtitle.Text = "USB is the source of truth for pull sync. Shadow is your local staging cache. You can also manually or automatically push local target changes back to the USB."
$headerSubtitle.Location = New-Object System.Drawing.Point(24, 48)
$headerSubtitle.Size = New-Object System.Drawing.Size(1036, 24)
$headerSubtitle.ForeColor = [System.Drawing.Color]::FromArgb(224, 235, 244)
$headerPanel.Controls.Add($headerSubtitle)

$alertPanel = New-Object System.Windows.Forms.Panel
$alertPanel.Location = New-Object System.Drawing.Point(18, 112)
$alertPanel.Size = New-Object System.Drawing.Size(1088, 74)
$alertPanel.BackColor = [System.Drawing.Color]::FromArgb(255, 244, 224)
$alertPanel.Visible = $false
$form.Controls.Add($alertPanel)

$alertTitle = New-Object System.Windows.Forms.Label
$alertTitle.Text = "Config issue detected"
$alertTitle.Location = New-Object System.Drawing.Point(18, 10)
$alertTitle.Size = New-Object System.Drawing.Size(320, 22)
$alertTitle.Font = New-Object System.Drawing.Font("Segoe UI Semibold", 10)
$alertTitle.ForeColor = [System.Drawing.Color]::FromArgb(132, 74, 0)
$alertPanel.Controls.Add($alertTitle)

$alertBody = New-Object System.Windows.Forms.Label
$alertBody.Location = New-Object System.Drawing.Point(18, 34)
$alertBody.Size = New-Object System.Drawing.Size(1050, 34)
$alertBody.ForeColor = [System.Drawing.Color]::FromArgb(104, 62, 0)
$alertPanel.Controls.Add($alertBody)

$tabs = New-Object System.Windows.Forms.TabControl
$tabs.Location = New-Object System.Drawing.Point(18, 198)
$tabs.Size = New-Object System.Drawing.Size(1088, 520)
$form.Controls.Add($tabs)

$tabOverview = New-Object System.Windows.Forms.TabPage
$tabOverview.Text = "Overview"
$tabOverview.BackColor = [System.Drawing.Color]::White
$tabs.TabPages.Add($tabOverview)

$tabJobs = New-Object System.Windows.Forms.TabPage
$tabJobs.Text = "Jobs"
$tabJobs.BackColor = [System.Drawing.Color]::White
$tabs.TabPages.Add($tabJobs)

$tabAdvanced = New-Object System.Windows.Forms.TabPage
$tabAdvanced.Text = "Advanced"
$tabAdvanced.BackColor = [System.Drawing.Color]::White
$tabs.TabPages.Add($tabAdvanced)

$groupDrive = New-Object System.Windows.Forms.GroupBox
$groupDrive.Text = "Drive"
$groupDrive.Location = New-Object System.Drawing.Point(18, 18)
$groupDrive.Size = New-Object System.Drawing.Size(500, 160)
$tabOverview.Controls.Add($groupDrive)

$groupDrive.Controls.Add((New-Label "USB Drive Letter" 18 32 130 22 $true))
$txtDrive = New-TextBox 18 56 70
$groupDrive.Controls.Add($txtDrive)
$toolTip.SetToolTip($txtDrive, "Single drive letter for the USB device, for example S or E.")

$chkEject = New-Object System.Windows.Forms.CheckBox
$chkEject.Text = "Eject after sync"
$chkEject.Location = New-Object System.Drawing.Point(132, 57)
$chkEject.Size = New-Object System.Drawing.Size(140, 24)
$groupDrive.Controls.Add($chkEject)
$toolTip.SetToolTip($chkEject, "Safely eject the USB after a successful sync run.")

$groupDrive.Controls.Add((New-Label "Config File" 18 92 110 22 $true))
$txtConfigPath = New-TextBox 18 116 450 $ConfigPath
$txtConfigPath.ReadOnly = $true
$groupDrive.Controls.Add($txtConfigPath)
$toolTip.SetToolTip($txtConfigPath, "This is the live config file the tray app reads.")

$groupBehavior = New-Object System.Windows.Forms.GroupBox
$groupBehavior.Text = "Sync Behavior"
$groupBehavior.Location = New-Object System.Drawing.Point(540, 18)
$groupBehavior.Size = New-Object System.Drawing.Size(512, 214)
$tabOverview.Controls.Add($groupBehavior)

$groupBehavior.Controls.Add((New-Label "Drive / Config Check Interval" 18 32 210 22 $true))
$numPoll = New-Object System.Windows.Forms.NumericUpDown
$numPoll.Location = New-Object System.Drawing.Point(18, 56)
$numPoll.Size = New-Object System.Drawing.Size(80, 24)
$numPoll.Minimum = 1
$numPoll.Maximum = 60
$numPoll.Value = 2
$groupBehavior.Controls.Add($numPoll)
$toolTip.SetToolTip($numPoll, "How often the tray app checks for drive insert/remove and config file updates.")

$groupBehavior.Controls.Add((New-InfoLabel "This timer is no longer used for live folder mirroring. Mounted USB and local-folder changes are handled by filesystem watchers." 118 50 368 38))

$chkSyncOnInsert = New-Object System.Windows.Forms.CheckBox
$chkSyncOnInsert.Text = "Pull from USB when inserted"
$chkSyncOnInsert.Location = New-Object System.Drawing.Point(18, 102)
$chkSyncOnInsert.Size = New-Object System.Drawing.Size(220, 24)
$groupBehavior.Controls.Add($chkSyncOnInsert)
$toolTip.SetToolTip($chkSyncOnInsert, "Run USB to PC sync one time when the configured drive first appears.")

$chkSyncWhileMounted = New-Object System.Windows.Forms.CheckBox
$chkSyncWhileMounted.Text = "Watch USB while mounted"
$chkSyncWhileMounted.Location = New-Object System.Drawing.Point(18, 132)
$chkSyncWhileMounted.Size = New-Object System.Drawing.Size(220, 24)
$groupBehavior.Controls.Add($chkSyncWhileMounted)
$toolTip.SetToolTip($chkSyncWhileMounted, "Use filesystem notifications to mirror new USB-side changes into shadow and the local target while the drive stays connected.")

$chkAutoSyncToUsb = New-Object System.Windows.Forms.CheckBox
$chkAutoSyncToUsb.Text = "Auto sync target back to USB"
$chkAutoSyncToUsb.Location = New-Object System.Drawing.Point(18, 162)
$chkAutoSyncToUsb.Size = New-Object System.Drawing.Size(230, 24)
$groupBehavior.Controls.Add($chkAutoSyncToUsb)
$toolTip.SetToolTip($chkAutoSyncToUsb, "Watch the local target folder and automatically push those changes back to the USB through the shadow cache. Turn this off if you only want manual push.")

$groupBehavior.Controls.Add((New-InfoLabel "If auto push is off, you can still use the tray menu action 'Sync to USB now' whenever you decide to publish the local target back onto the USB." 260 106 220 84))

$groupSummary = New-Object System.Windows.Forms.GroupBox
$groupSummary.Text = "Workflow"
$groupSummary.Location = New-Object System.Drawing.Point(18, 196)
$groupSummary.Size = New-Object System.Drawing.Size(500, 210)
$tabOverview.Controls.Add($groupSummary)

$groupSummary.Controls.Add((New-Label "Pull Path" 18 32 90 22 $true))
$groupSummary.Controls.Add((New-InfoLabel "USB source -> shadow cache -> local target. This is the normal ingest path." 18 58 440 32))
$groupSummary.Controls.Add((New-Label "Push Path" 18 98 90 22 $true))
$groupSummary.Controls.Add((New-InfoLabel "Local target -> shadow cache -> USB source. This runs manually or when auto push is enabled." 18 124 440 32))
$groupSummary.Controls.Add((New-Label "Delete Rules" 18 164 90 22 $true))
$groupSummary.Controls.Add((New-InfoLabel "Mirror deletes only follow the active source side for that sync direction. Local deletions do not remove USB files unless you run or enable push sync." 18 188 450 34))

$groupTips = New-Object System.Windows.Forms.GroupBox
$groupTips.Text = "Quick Tips"
$groupTips.Location = New-Object System.Drawing.Point(540, 252)
$groupTips.Size = New-Object System.Drawing.Size(512, 154)
$tabOverview.Controls.Add($groupTips)

$tipsText = New-Object System.Windows.Forms.TextBox
$tipsText.Location = New-Object System.Drawing.Point(18, 28)
$tipsText.Size = New-Object System.Drawing.Size(474, 106)
$tipsText.Multiline = $true
$tipsText.ReadOnly = $true
$tipsText.BorderStyle = 'None'
$tipsText.BackColor = [System.Drawing.Color]::White
$tipsText.Text = "1. Use one job per mirrored folder pair.`r`n2. Keep 'Watch USB while mounted' on if you want inserted drives to stay live.`r`n3. Leave 'Auto sync target back to USB' off if you want manual publish control.`r`n4. Shadow is the safety/staging layer; your configured target is still the live PC copy."
$groupTips.Controls.Add($tipsText)

$jobsHint = New-Object System.Windows.Forms.Label
$jobsHint.Text = "Each row maps one USB folder path to one local target folder."
$jobsHint.Location = New-Object System.Drawing.Point(18, 16)
$jobsHint.Size = New-Object System.Drawing.Size(500, 22)
$jobsHint.Font = New-Object System.Drawing.Font("Segoe UI Semibold", 9)
$tabJobs.Controls.Add($jobsHint)

$grid = New-Object System.Windows.Forms.DataGridView
$grid.Location = New-Object System.Drawing.Point(18, 46)
$grid.Size = New-Object System.Drawing.Size(1032, 326)
$grid.AllowUserToAddRows = $false
$grid.AllowUserToDeleteRows = $false
$grid.MultiSelect = $false
$grid.SelectionMode = "FullRowSelect"
$grid.AutoSizeColumnsMode = "Fill"
$grid.RowHeadersVisible = $false
$grid.EditMode = "EditOnEnter"
$grid.BackgroundColor = [System.Drawing.Color]::White
$grid.BorderStyle = "FixedSingle"
$grid.Columns.Add("Name", "Job Name") | Out-Null
$grid.Columns.Add("Source", "USB Source (relative)") | Out-Null
$grid.Columns.Add("Target", "Local Target (absolute)") | Out-Null
$mirrorColumn = New-Object System.Windows.Forms.DataGridViewCheckBoxColumn
$mirrorColumn.Name = "MirrorDeletes"
$mirrorColumn.HeaderText = "Mirror Deletes"
$grid.Columns.Add($mirrorColumn) | Out-Null
$grid.Columns["Name"].FillWeight = 18
$grid.Columns["Source"].FillWeight = 24
$grid.Columns["Target"].FillWeight = 42
$grid.Columns["MirrorDeletes"].FillWeight = 16
$tabJobs.Controls.Add($grid)
$toolTip.SetToolTip($grid, "Double-click cells to edit a job directly.")

$btnAdd = New-Object System.Windows.Forms.Button
$btnAdd.Text = "Add Job"
$btnAdd.Location = New-Object System.Drawing.Point(18, 388)
$btnAdd.Size = New-Object System.Drawing.Size(100, 32)
$tabJobs.Controls.Add($btnAdd)

$btnDuplicate = New-Object System.Windows.Forms.Button
$btnDuplicate.Text = "Duplicate"
$btnDuplicate.Location = New-Object System.Drawing.Point(126, 388)
$btnDuplicate.Size = New-Object System.Drawing.Size(100, 32)
$tabJobs.Controls.Add($btnDuplicate)

$btnRemove = New-Object System.Windows.Forms.Button
$btnRemove.Text = "Remove Job"
$btnRemove.Location = New-Object System.Drawing.Point(234, 388)
$btnRemove.Size = New-Object System.Drawing.Size(110, 32)
$tabJobs.Controls.Add($btnRemove)

$btnBrowseUsb = New-Object System.Windows.Forms.Button
$btnBrowseUsb.Text = "Browse USB Source"
$btnBrowseUsb.Location = New-Object System.Drawing.Point(360, 388)
$btnBrowseUsb.Size = New-Object System.Drawing.Size(145, 32)
$tabJobs.Controls.Add($btnBrowseUsb)

$btnBrowseTarget = New-Object System.Windows.Forms.Button
$btnBrowseTarget.Text = "Browse Local Target"
$btnBrowseTarget.Location = New-Object System.Drawing.Point(518, 388)
$btnBrowseTarget.Size = New-Object System.Drawing.Size(150, 32)
$tabJobs.Controls.Add($btnBrowseTarget)

$toolTip.SetToolTip($btnAdd, "Create a new job row.")
$toolTip.SetToolTip($btnDuplicate, "Clone the selected job so you can adjust it.")
$toolTip.SetToolTip($btnRemove, "Delete the selected job.")
$toolTip.SetToolTip($btnBrowseUsb, "Pick a folder from the configured USB drive and store it as a relative path.")
$toolTip.SetToolTip($btnBrowseTarget, "Pick the local live target folder on this PC.")

$jobsNotes = New-Object System.Windows.Forms.TextBox
$jobsNotes.Location = New-Object System.Drawing.Point(18, 438)
$jobsNotes.Size = New-Object System.Drawing.Size(1032, 48)
$jobsNotes.Multiline = $true
$jobsNotes.ReadOnly = $true
$jobsNotes.BorderStyle = 'None'
$jobsNotes.BackColor = [System.Drawing.Color]::White
$jobsNotes.Text = "Mirror Deletes: when enabled, files that disappear from the active source side are removed from the destination side during that sync direction. Pull sync uses USB as source. Push sync uses the local target as source."
$tabJobs.Controls.Add($jobsNotes)

$groupCache = New-Object System.Windows.Forms.GroupBox
$groupCache.Text = "Shadow Cache"
$groupCache.Location = New-Object System.Drawing.Point(18, 18)
$groupCache.Size = New-Object System.Drawing.Size(510, 190)
$tabAdvanced.Controls.Add($groupCache)

$chkShadow = New-Object System.Windows.Forms.CheckBox
$chkShadow.Text = "Use shadow cache"
$chkShadow.Location = New-Object System.Drawing.Point(18, 34)
$chkShadow.Size = New-Object System.Drawing.Size(180, 24)
$groupCache.Controls.Add($chkShadow)
$toolTip.SetToolTip($chkShadow, "Keep a local cache/staging copy for both pull and push operations.")

$chkClearShadow = New-Object System.Windows.Forms.CheckBox
$chkClearShadow.Text = "Clear shadow cache on eject"
$chkClearShadow.Location = New-Object System.Drawing.Point(18, 64)
$chkClearShadow.Size = New-Object System.Drawing.Size(210, 24)
$groupCache.Controls.Add($chkClearShadow)
$toolTip.SetToolTip($chkClearShadow, "Delete the shadow cache after a clean eject. Leave this off if you want the persistent cache to remain on the PC.")

$groupCache.Controls.Add((New-InfoLabel "Shadow helps avoid USB file lock problems and gives the app a stable local staging area for both directions. The shadow folder is not the live PC destination." 18 100 460 54))

$groupCompare = New-Object System.Windows.Forms.GroupBox
$groupCompare.Text = "Compare"
$groupCompare.Location = New-Object System.Drawing.Point(542, 18)
$groupCompare.Size = New-Object System.Drawing.Size(510, 190)
$tabAdvanced.Controls.Add($groupCompare)

$chkHash = New-Object System.Windows.Forms.CheckBox
$chkHash.Text = "Hash when metadata changes"
$chkHash.Location = New-Object System.Drawing.Point(18, 34)
$chkHash.Size = New-Object System.Drawing.Size(220, 24)
$groupCompare.Controls.Add($chkHash)
$toolTip.SetToolTip($chkHash, "Keep using hash comparison for metadata-changed files when needed.")

$groupCompare.Controls.Add((New-InfoLabel "The manifest stores file metadata and hashes. The app still skips unchanged files, but this option keeps stronger verification when timestamps or sizes change." 18 72 460 54))

$groupRecovery = New-Object System.Windows.Forms.GroupBox
$groupRecovery.Text = "Recovery and Utilities"
$groupRecovery.Location = New-Object System.Drawing.Point(18, 226)
$groupRecovery.Size = New-Object System.Drawing.Size(1034, 180)
$tabAdvanced.Controls.Add($groupRecovery)

$groupRecovery.Controls.Add((New-Label "Backup / Repair" 18 28 120 22 $true))
$recoveryText = New-Object System.Windows.Forms.TextBox
$recoveryText.Location = New-Object System.Drawing.Point(18, 54)
$recoveryText.Size = New-Object System.Drawing.Size(990, 54)
$recoveryText.Multiline = $true
$recoveryText.ReadOnly = $true
$recoveryText.BorderStyle = 'None'
$recoveryText.BackColor = [System.Drawing.Color]::White
$recoveryText.Text = "If the config becomes unreadable, the tray app can back it up, regenerate a safe default file, and auto-open this wizard so you can repair settings without hand-editing JSON."
$groupRecovery.Controls.Add($recoveryText)

$btnOpenRaw = New-Object System.Windows.Forms.Button
$btnOpenRaw.Text = "Open Raw Config"
$btnOpenRaw.Location = New-Object System.Drawing.Point(18, 124)
$btnOpenRaw.Size = New-Object System.Drawing.Size(130, 32)
$groupRecovery.Controls.Add($btnOpenRaw)

$btnLoadDefaults = New-Object System.Windows.Forms.Button
$btnLoadDefaults.Text = "Load Defaults"
$btnLoadDefaults.Location = New-Object System.Drawing.Point(160, 124)
$btnLoadDefaults.Size = New-Object System.Drawing.Size(120, 32)
$groupRecovery.Controls.Add($btnLoadDefaults)

$btnOpenBackup = New-Object System.Windows.Forms.Button
$btnOpenBackup.Text = "Open Backup"
$btnOpenBackup.Location = New-Object System.Drawing.Point(292, 124)
$btnOpenBackup.Size = New-Object System.Drawing.Size(110, 32)
$btnOpenBackup.Enabled = -not [string]::IsNullOrWhiteSpace($RecoveryBackupPath)
$groupRecovery.Controls.Add($btnOpenBackup)

$backupLabel = New-Object System.Windows.Forms.Label
$backupLabel.Location = New-Object System.Drawing.Point(420, 130)
$backupLabel.Size = New-Object System.Drawing.Size(580, 24)
$backupLabel.ForeColor = [System.Drawing.Color]::FromArgb(84, 92, 104)
$backupLabel.Text = if ([string]::IsNullOrWhiteSpace($RecoveryBackupPath)) { "No recovery backup for this session." } else { "Recovery backup: $RecoveryBackupPath" }
$groupRecovery.Controls.Add($backupLabel)

$toolTip.SetToolTip($btnOpenRaw, "Open the raw JSON config in Notepad.")
$toolTip.SetToolTip($btnLoadDefaults, "Replace the form values with a safe starter config. This does not save until you click Save.")
$toolTip.SetToolTip($btnOpenBackup, "Open the repaired backup copy of the invalid config.")

$btnSave = New-Object System.Windows.Forms.Button
$btnSave.Text = "Save Setup"
$btnSave.Location = New-Object System.Drawing.Point(898, 734)
$btnSave.Size = New-Object System.Drawing.Size(100, 34)
$form.Controls.Add($btnSave)

$btnCancel = New-Object System.Windows.Forms.Button
$btnCancel.Text = "Cancel"
$btnCancel.Location = New-Object System.Drawing.Point(1006, 734)
$btnCancel.Size = New-Object System.Drawing.Size(100, 34)
$form.Controls.Add($btnCancel)

$lblStatus = New-Object System.Windows.Forms.Label
$lblStatus.Location = New-Object System.Drawing.Point(22, 740)
$lblStatus.Size = New-Object System.Drawing.Size(840, 24)
$lblStatus.ForeColor = [System.Drawing.Color]::FromArgb(76, 86, 98)
$form.Controls.Add($lblStatus)

$form.AcceptButton = $btnSave
$form.CancelButton = $btnCancel

if (-not [string]::IsNullOrWhiteSpace($ErrorMessage)) {
    $alertPanel.Visible = $true
    $alertBody.Text = if ($RecoveredDefault) {
        if ([string]::IsNullOrWhiteSpace($RecoveryBackupPath)) {
            "The existing config could not be loaded, so a safe default config was generated. Review the settings below and save when ready. Error: $ErrorMessage"
        } else {
            "The existing config could not be loaded, so it was backed up and replaced with a safe default. Backup: $RecoveryBackupPath  Error: $ErrorMessage"
        }
    } else {
        "The config loaded with an error or failed validation. Review the settings below and save a corrected version. Error: $ErrorMessage"
    }
}

Apply-DefaultValues

if (Test-Path -LiteralPath $ConfigPath) {
    try {
        $config = Get-Content -LiteralPath $ConfigPath -Raw | ConvertFrom-Json
        if ($null -ne $config.drive) {
            $txtDrive.Text = [string]$config.drive.letter
            $chkEject.Checked = [bool]$config.drive.eject_after_sync
        }
        if ($null -ne $config.app) {
            if ($null -ne $config.app.poll_interval_seconds) { $numPoll.Value = [decimal]$config.app.poll_interval_seconds }
            if ($null -ne $config.app.sync_on_insert) { $chkSyncOnInsert.Checked = [bool]$config.app.sync_on_insert }
            if ($null -ne $config.app.sync_while_mounted) { $chkSyncWhileMounted.Checked = [bool]$config.app.sync_while_mounted }
            if ($null -ne $config.app.auto_sync_to_usb) { $chkAutoSyncToUsb.Checked = [bool]$config.app.auto_sync_to_usb }
        }
        if ($null -ne $config.cache) {
            if ($null -ne $config.cache.shadow_copy) { $chkShadow.Checked = [bool]$config.cache.shadow_copy }
            if ($null -ne $config.cache.clear_shadow_on_eject) { $chkClearShadow.Checked = [bool]$config.cache.clear_shadow_on_eject }
        }
        if ($null -ne $config.compare) {
            if ($null -ne $config.compare.hash_on_metadata_change) { $chkHash.Checked = [bool]$config.compare.hash_on_metadata_change }
        }
        if ($null -ne $config.jobs) {
            foreach ($job in $config.jobs) {
                $grid.Rows.Add([string]$job.name, [string]$job.source, [string]$job.target, [bool]$job.mirror_deletes) | Out-Null
            }
        }
    } catch {
        Apply-DefaultValues
        Set-Status "Loaded default values because the config could not be read."
    }
}

$grid.add_SelectionChanged({
    $selected = Get-SelectedJobRow
    if ($selected -ne $null) {
        Set-Status ("Selected job: " + [string]$selected.Cells["Name"].Value)
    }
})

$btnAdd.Add_Click({
    $grid.Rows.Add("", "", "", $true) | Out-Null
    $grid.CurrentCell = $grid.Rows[$grid.Rows.Count - 1].Cells["Name"]
    $grid.BeginEdit($true) | Out-Null
    Set-Status "Added a new job row."
})

$btnDuplicate.Add_Click({
    $selected = Get-SelectedJobRow
    if ($selected -eq $null) {
        Show-ErrorDialog "Select a job row first."
        return
    }

    $name = [string]$selected.Cells["Name"].Value
    $source = [string]$selected.Cells["Source"].Value
    $target = [string]$selected.Cells["Target"].Value
    $mirrorDeletes = [bool]$selected.Cells["MirrorDeletes"].Value
    $grid.Rows.Add("$name Copy", $source, $target, $mirrorDeletes) | Out-Null
    Set-Status "Duplicated the selected job."
})

$btnRemove.Add_Click({
    $selected = Get-SelectedJobRow
    if ($selected -eq $null) {
        Show-ErrorDialog "Select a job row first."
        return
    }

    $grid.Rows.RemoveAt($selected.Index)
    Set-Status "Removed the selected job."
})

$btnBrowseUsb.Add_Click({
    $selected = Get-SelectedJobRow
    if ($selected -eq $null) {
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
        $selected.Cells["Source"].Value = $relative
        Set-Status "USB source updated."
    }
})

$btnBrowseTarget.Add_Click({
    $selected = Get-SelectedJobRow
    if ($selected -eq $null) {
        Show-ErrorDialog "Select a job row first."
        return
    }

    $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
    $dialog.Description = "Choose the local target folder."
    $dialog.UseDescriptionForTitle = $true
    if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
        $selected.Cells["Target"].Value = $dialog.SelectedPath
        Set-Status "Local target updated."
    }
})

$btnOpenRaw.Add_Click({
    Start-Process notepad.exe -ArgumentList @($ConfigPath) | Out-Null
})

$btnOpenBackup.Add_Click({
    if (-not [string]::IsNullOrWhiteSpace($RecoveryBackupPath) -and (Test-Path -LiteralPath $RecoveryBackupPath)) {
        Start-Process notepad.exe -ArgumentList @($RecoveryBackupPath) | Out-Null
    }
})

$btnLoadDefaults.Add_Click({
    Apply-DefaultValues
    Set-Status "Loaded default starter values into the form. Save to make them live."
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
                name = $name.Trim()
                source = $source.Trim()
                target = $target.Trim()
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
        Set-Status "Config saved."
        [System.Windows.Forms.MessageBox]::Show(
            "Config saved. The tray app will pick it up automatically.",
            "USB Mirror Sync Setup",
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Information
        ) | Out-Null
        $form.Close()
    } catch {
        Show-ErrorDialog $_.Exception.Message
    }
})

Set-Status "Review the setup and click Save Setup when ready."
[void]$form.ShowDialog()
