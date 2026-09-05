let me;

const refresh = async () => {
  me = await api("/api/me");
  renderMe();
};

function fileCells(f) {
  const nameTd = el("td", "auto-style10");
  const a = el("a");
  a.href = "https://" + me.domain + "/" + encodeURIComponent(f.name);
  a.target = "_blank";
  a.appendChild(el("strong", "", f.name));
  nameTd.appendChild(a);

  const btnTd = el("td", "auto-style10");
  const form = el("form");
  form.style.margin = "0";
  const btn = el("input");
  btn.type = "submit";
  btn.value = "CANCELLA";
  btn.addEventListener("click", async (ev) => {
    ev.preventDefault();
    try {
      await api("/api/files/delete", { json: { name: f.name } });
      await refresh();
    } catch (e) {
      showMsg("dash-err", e.message);
    }
  });
  form.appendChild(btn);
  btnTd.appendChild(form);

  return [nameTd, el("td", "auto-style10 small", fmtSize(f.size_bytes)), btnTd];
}

function renderMe() {
  $("dash-title").textContent = me.domain;
  $("site-link").href = "https://" + me.domain + "/";
  const pct = Math.min(
    100,
    Math.floor((me.used_bytes * 100) / Math.max(1, me.quota_bytes)),
  );
  $("used-line").textContent =
    "Spazio usato: " +
    fmtSize(me.used_bytes) +
    " / " +
    fmtSize(me.quota_bytes) +
    " (" +
    pct +
    "%)";
  $("traffic-line").textContent =
    "Visite: " + me.visits.toLocaleString("it-IT");
  $("meta-name").value = me.name;
  $("meta-desc").value = me.description;
  fillRows(
    $("file-list"),
    me.files,
    (f) => rowOf(fileCells(f)),
    "Nessun file caricato.",
  );
}

onPage("dashboard", async () => {
  try {
    await refresh();
  } catch (e) {
    location.href = "/index.html";
    return;
  }

  wireForm("upload-form", "dash-err", async () => {
    showMsg("dash-msg", "caricamento in corso...");
    const out = await api("/api/files", {
      method: "POST",
      body: new FormData($("upload-form")),
    });
    showMsg(
      "dash-msg",
      out.errors.length
        ? out.saved + " file caricati, con errori:"
        : out.saved + " file caricati!",
    );
    if (out.errors.length) showMsg("dash-err", out.errors[0]);
    await refresh();
  });

  wireForm("meta-form", "dash-err", async () => {
    await api("/api/me/meta", {
      json: {
        name: $("meta-name").value,
        description: $("meta-desc").value,
      },
    });
    showMsg("dash-msg", "info salvate!");
    await refresh();
  });

  $("logout-btn").addEventListener("click", async () => {
    await api("/api/logout", { json: {} });
    location.href = "/index.html";
  });

  $("delete-btn").addEventListener("click", async () => {
    if (
      !confirm(
        "Cancellare il tuo sito e tutti i suoi file? Azione irreversibile!",
      )
    )
      return;
    try {
      await api("/api/me/delete", { json: {} });
      location.href = "/index.html";
    } catch (e) {
      showMsg("dash-err", e.message);
    }
  });
});
