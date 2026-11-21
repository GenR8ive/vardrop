# Vardrop - smoke test: health, login, cluster/group/entry CRUD, search, value,
# PAT, secret PIN kapısı, audit.
#
# Kullanim:
#   $env:VARDROP_ADMIN_PASSWORD = "..."   (yoksa reset-password ile yeni uret)
#   powershell -File scripts\smoke.ps1
$ErrorActionPreference = "Stop"
$base = "http://127.0.0.1:8080"
$adminPassword = $env:VARDROP_ADMIN_PASSWORD
if (-not $adminPassword) {
  Write-Output "SMOKE-FAIL: VARDROP_ADMIN_PASSWORD ortam degiskeni yok (reset-password ile sifreyi sifirlayip koyun)"
  exit 1
}

# wait up to 4 min for the server (cargo run may still be building)
$up = $false
for ($i = 0; $i -lt 80; $i++) {
  try { $null = Invoke-RestMethod "$base/api/health" -TimeoutSec 2; $up = $true; break }
  catch { Start-Sleep -Seconds 3 }
}
if (-not $up) { Write-Output "SMOKE-FAIL: server did not come up"; exit 1 }
$health = Invoke-RestMethod "$base/api/health"
Write-Output ("health: {0} v{1}" -f $health.status, $health.version)

$login  = Invoke-RestMethod "$base/api/auth/login" -Method Post -ContentType "application/json" -Body (@{ username = "admin"; password = $adminPassword } | ConvertTo-Json)
$H = @{ Authorization = "Bearer " + $login.token }
Write-Output ("login: {0} role={1}" -f $login.user.username, $login.user.role)

# bad password check
$badFail = $false
try {
  Invoke-RestMethod "$base/api/auth/login" -Method Post -ContentType "application/json" -Body '{"username":"admin","password":"wrong!!"}' | Out-Null
} catch { $badFail = $true }
Write-Output ("login-wrong-password-rejected: {0}" -f $badFail)

$c1 = Invoke-RestMethod "$base/api/clusters" -Method Post -Headers $H -ContentType "application/json" -Body (@{ name = "Variables"; description = "ortam degiskenleri"; color = "#6366f1" } | ConvertTo-Json)
Write-Output ("cluster: {0} slug={1}" -f $c1.name, $c1.slug)

$g1 = Invoke-RestMethod "$base/api/clusters/$($c1.id)/groups" -Method Post -Headers $H -ContentType "application/json" -Body '{"name":"Development"}'
$g2 = Invoke-RestMethod "$base/api/clusters/$($c1.id)/groups" -Method Post -Headers $H -ContentType "application/json" -Body (@{ name = "Production"; parent_id = $g1.id } | ConvertTo-Json)
Write-Output ("groups: {0}, {1} (child of dev)" -f $g1.name, $g2.name)

$e1 = Invoke-RestMethod "$base/api/entries" -Method Post -Headers $H -ContentType "application/json" -Body (@{ cluster_id = $c1.id; group_id = $g1.id; name = "SQL_CONNECTION_STRING"; kind = "json"; value = '{"host":"db.dev.local","port":5432}' } | ConvertTo-Json)
$e2 = Invoke-RestMethod "$base/api/entries" -Method Post -Headers $H -ContentType "application/json" -Body (@{ cluster_id = $c1.id; group_id = $g2.id; name = "API_KEY"; kind = "text"; is_secret = $true; value = "sk-test-9876543210" } | ConvertTo-Json)
Write-Output ("entries: {0} (json) / {1} (secret, preview={2})" -f $e1.name, $e2.name, $e2.preview)

# update value -> version bump
$e1b = Invoke-RestMethod "$base/api/entries/$($e1.id)" -Method Patch -Headers $H -ContentType "application/json" -Body (@{ value = '{"host":"db.dev2.local","port":5432}' } | ConvertTo-Json)
Write-Output ("entry-patch: version {0} -> {1}" -f $e1.current_version, $e1b.current_version)

$search = Invoke-RestMethod "$base/api/entries?q=API&limit=10" -Headers $H
Write-Output ("search q=API: total={0} first={1} preview={2}" -f $search.total, $search.results[0].name, $search.results[0].preview)

$val = Invoke-RestMethod "$base/api/entries/$($e2.id)/value" -Headers $H
Write-Output ("secret-value: {0}" -f $val.value)

# restore v1 (e1b is v2)
$rest = Invoke-RestMethod "$base/api/entries/$($e1.id)/versions/1/restore" -Method Post -Headers $H
Write-Output ("restore: version -> {0}" -f $rest.current_version)

# PAT: uret + read kapsamiyla kullan
$t = Invoke-RestMethod "$base/api/auth/tokens" -Method Post -Headers $H -ContentType "application/json" -Body '{"name":"probe","scopes":["read"],"expires_days":7}'
$TH = @{ Authorization = "Bearer " + $t.token }
$tokSearch = Invoke-RestMethod "$base/api/entries?limit=3" -Headers $TH
Write-Output ("pat-created: id={0} read-scope-search-ok={1}" -f $t.id, ($tokSearch.total -ge 2))

# TTL dogrulamasi: 0 gun reddedilmeli
$ttlRejected = $false
try {
  Invoke-RestMethod "$base/api/auth/tokens" -Method Post -Headers $H -ContentType "application/json" -Body '{"name":"bad","scopes":["read"],"expires_days":0}' | Out-Null
} catch { $ttlRejected = $true }
Write-Output ("pat-ttl-zero-rejected: {0}" -f $ttlRejected)

# secret PIN kapisi: pin yok -> serbest; pin koy -> 403 pin_required; unlock -> serbest; kaldir
$pinBefore = Invoke-RestMethod "$base/api/auth/pin" -Headers $H
Invoke-RestMethod "$base/api/auth/pin" -Method Put -Headers $H -ContentType "application/json" -Body '{"new_pin":"4321"}' | Out-Null
$pinBlocked = $false
$pinCode = ""
try { Invoke-RestMethod "$base/api/entries/$($e2.id)/value" -Headers $H | Out-Null }
catch {
  $pinBlocked = $true
  $pinCode = (Invoke-RestMethod -Uri "$base/api/entries/$($e2.id)/value" -Headers $H -SkipHttpErrorCheck).code
}
Write-Output ("pin-gate-blocked: {0} code={1}" -f $pinBlocked, $pinCode)
$u = Invoke-RestMethod "$base/api/auth/unlock" -Method Post -Headers $H -ContentType "application/json" -Body '{"pin":"4321"}'
$afterUnlock = Invoke-RestMethod "$base/api/entries/$($e2.id)/value" -Headers $H
Write-Output ("pin-unlock: window={0}s secret-ok={1}" -f $u.expires_in, ($afterUnlock.value.Length -gt 0))
Invoke-RestMethod "$base/api/auth/pin" -Method Put -Headers $H -ContentType "application/json" -Body '{"current_pin":"4321","new_pin":""}' | Out-Null
$pinAfter = Invoke-RestMethod "$base/api/auth/pin" -Headers $H
Write-Output ("pin-cleared: set={0}" -f $pinAfter.set)

# kaldirilan client/connect uclari 404/405 vermeli (SPA fallback)
$clientGone = $false
try { Invoke-RestMethod "$base/api/client/files" -Headers $H | Out-Null }
catch { $clientGone = $true }
Write-Output ("client-endpoint-gone: {0}" -f $clientGone)

$audit = Invoke-RestMethod "$base/api/audit?limit=20" -Headers $H
$actions = ($audit | ForEach-Object { $_.action } | Select-Object -Unique) -join ","
Write-Output ("audit-actions: {0}" -f $actions)

Write-Output "SMOKE-OK"
