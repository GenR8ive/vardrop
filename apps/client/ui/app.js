// Vardrop popup mantığı — window.__TAURI__ global (withGlobalTauri: true).
const { invoke } = window.__TAURI__.core;

const setupEl = document.getElementById("setup");
const searcherEl = document.getElementById("searcher");
const qEl = document.getElementById("q");
const listEl = document.getElementById("list");
const statusEl = document.getElementById("status");
const setupMsg = document.getElementById("setup-msg");

let settings = null;
let timer = null;

// --- gezinme durumu ---
// mode: "groups" (ağaç) | "entries" (drill veya arama sonuçları)
// rows: ekran satırları; {t:"hdr",label,color} | {t:"group",id,name,depth,count} | {t:"entry",entry}
let mode = "groups";
let rows = [];
let sel = 0;
let nav = []; // /api/nav önbelleği: [{id,name,slug,color,entry_count,roots:[GroupNode]..}]
let drillGid = null; // entries moduna x-dönüş için

function setStatus(msg, cls) {
  statusEl.textContent = msg;
  statusEl.className = "status" + (cls ? " " + cls : "");
}

function api() {
  return (settings.server || "").replace(/\/+$/, "");
}

init();

function httpWarning(server) {
  try {
    const u = new URL(server);
    const host = u.hostname;
    const local = host === "localhost" || host === "127.0.0.1" || host === "::1";
    if (u.protocol === "http:" && !local) {
      return "uyarı: http — token ağda düz metin gider. Hosting'de https kullanın.";
    }
  } catch (_) {}
  return "";
}

async function forgetSettings() {
  try {
    await invoke("clear_settings");
    settings = await invoke("get_settings");
    document.getElementById("setup-server").value = "";
    document.getElementById("setup-token").value = "";
    setupMsg.textContent = "kayıtlı bağlantı silindi";
    setupMsg.className = "";
  } catch (e) {
    setupMsg.textContent = "silinemedi: " + (e && e.message ? e.message : e);
  }
}

async function init() {
  settings = await invoke("get_settings");
  document.getElementById("open-setup").onclick = () => showSetup();
  document.getElementById("setup-forget").onclick = forgetSettings;
  if (!settings.server || !settings.has_token) {
    showSetup();
  } else {
    startSearch();
  }
}

// ---------- ortak istek ----------
async function apiGet(path) {
  let res;
  try {
    res = await fetch(api() + path, {
      headers: { Authorization: "Bearer " + settings.token },
      cache: "no-store",
    });
  } catch (e) {
    throw new Error("sunucuya erişilemedi: " + (e && e.message ? e.message : "?"));
  }
  if (res.status === 401) {
    setStatus("bağlantı geçersiz / süresi doldu — kurulum ekranı açılıyor", "err");
    setTimeout(showSetup, 1200);
    throw new Error("401");
  }
  if (!res.ok) {
    throw new Error("HTTP " + res.status);
  }
  try {
    return await res.json();
  } catch (perr) {
    const body = await res.text().catch(() => "");
    throw new Error("beklenmeyen cevap (" + body.slice(0, 60) + ")");
  }
}

// ---------- ilk kurulum: sunucu adresi + PAT ----------
function showSetup() {
  setupEl.classList.remove("hidden");
  searcherEl.classList.add("hidden");
  const serverEl = document.getElementById("setup-server");
  const tokenEl = document.getElementById("setup-token");
  // sunucu adresini hatırla (token iptal olup kurulum yeniden açılırsa)
  if (!serverEl.value && settings.server) serverEl.value = settings.server;

  const btn = document.getElementById("setup-save");
  btn.onclick = () => {
    const server = serverEl.value.trim().replace(/\/+$/, "");
    const token = tokenEl.value.trim();
    if (!/^https?:\/\/\S+$/i.test(server)) {
      setupMsg.textContent = "sunucu adresi geçersiz (ör. http://127.0.0.1:8080)";
      return;
    }
    if (!token) {
      setupMsg.textContent = "PAT boş — panel → Tokenlar → üret";
      return;
    }
    connectWithPat(server, token);
  };
}

function setBtnBusy(b) {
  const btn = document.getElementById("setup-save");
  btn.disabled = b;
  btn.textContent = b ? "bağlanıyor…" : "Bağlan";
}

/// PAT akışı: token'ı sunucuda doğrula (/api/auth/me), geçerliyse kaydet.
/// Token read kapsamlıdır; panelden istendiğinde iptal edilir.
async function connectWithPat(server, token) {
  setBtnBusy(true);
  try {
    const w = httpWarning(server);
    setupMsg.textContent = w || "doğrulanıyor…";
    let res;
    try {
      res = await fetch(server + "/api/auth/me", {
        headers: { Authorization: "Bearer " + token },
        cache: "no-store",
      });
    } catch (e) {
      throw new Error("sunucuya erişilemedi: " + (e && e.message ? e.message : "?"));
    }
    if (res.status === 401 || res.status === 403) {
      throw new Error("PAT reddedildi — panelden yeni token üretin");
    }
    if (!res.ok) {
      throw new Error("HTTP " + res.status);
    }
    await invoke("save_settings", { server, token });
    settings = await invoke("get_settings");
    if (!settings.server || !settings.has_token) {
      throw new Error("kayıt doğrulanamadı");
    }
    setupMsg.textContent = "";
    startSearch();
  } catch (e) {
    setupMsg.textContent = (e && e.message) || "bağlantı başarısız";
  } finally {
    setBtnBusy(false);
  }
}

// ---------- arama ekranı ----------
async function startSearch() {
  setupEl.classList.add("hidden");
  searcherEl.classList.remove("hidden");

  qEl.addEventListener("input", () => {
    clearTimeout(timer);
    timer = setTimeout(search, 130);
  });
  qEl.addEventListener("keydown", onKey);
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") invoke("hide_popup");
  });
  // her açılışta formu sıfırla → yerine: durumu SAKLA
  // (arama sorgusu + sonuçlar kapanınca korunur; yeniden açınca aynı kalır)
  window.addEventListener("focus", () => {
    if (mode === "groups" && !qEl.value) {
      resetToGroups(true); // nav önbelleğini tazeler (yeni gruptan haberdar et)
    } else {
      render(); // mevcut durum korunur
    }
    qEl.focus();
  });

  resetToGroups(); // nav önbelleği boşken placeholder görünür; loadNav dolunca yeniden çizilir
  qEl.focus();
  await loadNav();
  resetToGroups(true);
}

async function loadNav() {
  try {
    nav = await apiGet("/api/nav");
  } catch (e) {
    if (e.message === "401") return;
    setStatus("grup ağacı yüklenemedi: " + e.message, "err");
  }
}

// q boşaltılır (true:FlatRows yeniden üret + çiz)
function resetToGroups(refresh) {
  qEl.value = "";
  mode = "groups";
  drillGid = null;
  rows = refresh ? buildGroupRows() : rows;
  sel = nextSel(rows, 0, 1);
  render();
  setStatus(rows.length ? "gruplarda gezin — yazınca değişken arar" : "yükleniyor…");
}

// kluster başlıkları + DFS grup ağacı
function buildGroupRows() {
  const out = [];
  for (const c of nav || []) {
    out.push({ t: "hdr", label: c.name, color: c.color });
    const dfs = (nodes, depth) => {
      for (const n of nodes || []) {
        out.push({ t: "group", id: n.id, name: n.name, depth, count: n.entry_count });
        dfs(n.children, depth + 1);
      }
    };
    dfs(c.roots, 0);
  }
  return out;
}

// arama/drill sonuçlarını path başlıklarıyla hiyerarşiye oturt
function buildEntryRows(list) {
  const out = [];
  let last = null;
  for (const entry of list || []) {
    const cluster = (nav || []).find((c) => c.id === entry.cluster_id);
    const label = (cluster ? cluster.name : "") + (entry.path ? " / " + entry.path : "");
    if (label !== last) {
      out.push({ t: "hdr", label: label || "—", color: cluster ? cluster.color : null });
      last = label;
    }
    out.push({ t: "entry", entry });
  }
  return out;
}

async function search() {
  const term = qEl.value.trim();
  if (!settings || !settings.has_token) {
    setStatus("kurulum eksik — kurulum ekranı açılıyor", "err");
    setTimeout(showSetup, 1200);
    return;
  }
  if (!term) {
    resetToGroups(true);
    return;
  }
  try {
    const data = await apiGet("/api/entries?q=" + encodeURIComponent(term) + "&limit=25");
    rows = buildEntryRows(data.results || []);
    mode = "entries";
    drillGid = null;
    sel = nextSel(rows, 0, 1);
    render();
    if ((data.results || []).length === 0) setStatus("sonuç yok", "err");
    else setStatus(data.total + " sonuç · Enter değişken kopyalar · input temizle = ağaca dön");
  } catch (e) {
    if (e.message === "401") return;
    rows = [];
    render();
    setStatus("arama başarısız: " + e.message, "err");
  }
}

// ---------- klavye & seçim ----------
function nextSel(rows, from, dir) {
  let i = from;
  for (;;) {
    i += dir;
    if (i < 0) i = rows.length - 1;
    if (i >= rows.length) i = 0;
    if (!rows[i] || rows[i].t !== "hdr") return i;
  }
}

function onKey(e) {
  if (e.key === "ArrowDown") {
    if (rows.length) {
      sel = nextSel(rows, sel, 1);
      render();
    }
    e.preventDefault();
  } else if (e.key === "ArrowUp") {
    if (rows.length) {
      sel = nextSel(rows, sel, -1);
      render();
    }
    e.preventDefault();
  } else if (e.key === "Enter") {
    activate(rows[sel]);
    e.preventDefault();
  } else if (e.key === "Backspace") {
    // input boşsa entries-modundan ağaca geri
    if (qEl.value === "" && mode === "entries") {
      e.preventDefault();
      gotoGroupsBack();
    }
  }
}

function gotoGroupsBack() {
  const gid = drillGid;
  rows = buildGroupRows();
  mode = "groups";
  drillGid = null;
  sel = nextSel(rows, 0, 1);
  const idx = rows.findIndex((r) => r.t === "group" && r.id === gid);
  if (idx >= 0) sel = idx;
  render();
  setStatus("gruplarda gezin — yazınca değişken arar");
}

async function activate(row) {
  if (!row) return;
  if (row.t === "group") await drillInto(row);
  else if (row.t === "entry") await pickEntry(row.entry);
}

async function drillInto(row) {
  try {
    const data = await apiGet("/api/entries?group_id=" + encodeURIComponent(row.id) + "&limit=25");
    rows = buildEntryRows(data.results || []);
    mode = "entries";
    drillGid = row.id;
    sel = nextSel(rows, 0, 1);
    render();
    if ((data.results || []).length === 0)
      setStatus("bu grupta değişken yok · Backspace = ağaca dön", "err");
    else setStatus(data.total + " değişken · Enter kopyalar · Backspace = ağaca dön");
  } catch (e) {
    if (e.message === "401") return;
    setStatus("grup açılamadı: " + e.message, "err");
  }
}

// ---------- render ----------
function esc(s) {
  return String(s == null ? "" : s).replace(/[&<>"]/g, (c) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;",
  })[c]);
}

function render() {
  listEl.innerHTML = rows
    .map((r, i) => {
      const isSel = i === sel;
      if (r.t === "hdr") {
        const color = /^#[0-9a-fA-F]{3,8}$/.test(r.color || "") ? r.color : "";
        const dot = color
          ? '<span class="dot" style="background:' + color + '"></span>'
          : '<span class="dot"></span>';
        return '<li class="hdr' + (isSel ? " sel" : "") + '">' + dot + esc(r.label) + "</li>";
      }
      if (r.t === "group") {
        return (
          '<li class="' + (isSel ? "sel" : "") + '" data-i="' + i + '" style="padding-left:' + (10 + r.depth * 16) + 'px">' +
          '<div class="name"><span class="gicon">▸</span>' + esc(r.name) +
          '<span class="cnt">' + r.count + "</span></div>" +
          "</li>"
        );
      }
      const entry = r.entry;
      const chips =
        '<span class="chip">' + esc(entry.kind) + "</span>" +
        (entry.is_secret ? '<span class="chip secret">secret</span>' : "");
      return (
        '<li class="' + (isSel ? "sel" : "") + '" data-i="' + i + '">' +
        '<div class="name">' + esc(entry.name) + chips + "</div>" +
        '<div class="prev">' + esc(entry.preview || "") + "</div>" +
        "</li>"
      );
    })
    .join("");

  listEl.querySelectorAll("li[data-i]").forEach((li) => {
    li.onmouseenter = () => {
      const i = Number(li.dataset.i);
      if (rows[i] && rows[i].t !== "hdr") {
        sel = i;
        render();
      }
    };
    li.onclick = () => activate(rows[Number(li.dataset.i)]);
  });
  const cur = listEl.querySelector("li.sel");
  if (cur) cur.scrollIntoView({ block: "nearest" });
}

// ---------- kopyalama ----------

// secret PIN kapısı: 403 pin_required → PIN kutusu → unlock → tekrar dene
function pinboxEls() {
  return {
    box: document.getElementById("pinbox"),
    input: document.getElementById("pin-input"),
    msg: document.getElementById("pin-msg"),
    ok: document.getElementById("pin-ok"),
    cancel: document.getElementById("pin-cancel"),
  };
}

function showPinBox(retry) {
  const p = pinboxEls();
  p.box.classList.remove("hidden");
  p.input.value = "";
  p.msg.textContent = "secret için PIN gerekli";
  p.ok.onclick = async () => {
    const pin = p.input.value.trim();
    if (!pin) {
      p.msg.textContent = "PIN boş";
      return;
    }
    p.ok.disabled = true;
    try {
      const res = await fetch(api() + "/api/auth/unlock", {
        method: "POST",
        headers: { "Content-Type": "application/json", Authorization: "Bearer " + settings.token },
        body: JSON.stringify({ pin }),
      });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(data.error || "HTTP " + res.status);
      hidePinBox();
      p.ok.disabled = false;
      await retry();
    } catch (e) {
      p.msg.textContent = (e && e.message) || "PIN doğrulanamadı";
    } finally {
      p.ok.disabled = false;
    }
  };
  p.cancel.onclick = () => hidePinBox();
  p.input.focus();
}

function hidePinBox() {
  const p = pinboxEls();
  p.box.classList.add("hidden");
  p.input.value = "";
  p.msg.textContent = "";
}

async function pickEntry(entry) {
  if (!entry) return;
  if (entry.kind === "binary" || entry.kind === "bundle") {
    setStatus("dosya tabanlı içerik — panelden indirebilirsiniz", "err");
    return;
  }
  await fetchValueAndCopy(entry);
}

/// Değeri çeker, panoya kopyalar. 403 pin_required → PIN kutusu → unlock → tekrar.
async function fetchValueAndCopy(entry) {
  let res;
  try {
    res = await fetch(api() + "/api/entries/" + entry.id + "/value", {
      headers: { Authorization: "Bearer " + settings.token },
      cache: "no-store",
    });
  } catch (_) {
    setStatus("sunucuya erişilemedi", "err");
    return;
  }
  if (res.status === 401) {
    setStatus("bağlantı geçersiz — kurulum ekranı açılıyor", "err");
    setTimeout(showSetup, 1200);
    return;
  }
  if (res.status === 403) {
    const data = await res.json().catch(() => ({}));
    if (data.code === "pin_required") {
      setStatus("secret — PIN gerekli", "err");
      showPinBox(() => fetchValueAndCopy(entry));
      return;
    }
    setStatus("yetki yok (403)", "err");
    return;
  }
  if (!res.ok) {
    setStatus("değer alınamadı (HTTP " + res.status + ")", "err");
    return;
  }
  const data = await res.json();
  const text = data.value || "";
  hidePinBox();
  try {
    await navigator.clipboard.writeText(text);
  } catch (_) {
    await invoke("copy_text", { text });
  }
  setStatus("kopyalandı: " + entry.name, "ok");
  await new Promise((res2) => setTimeout(res2, 250));
  await invoke("hide_popup");
}
