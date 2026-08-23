function actionButton(label, onClick) {
  const form = el("form");
  form.style.margin = "0";
  form.style.display = "inline";
  const btn = el("input");
  btn.type = "submit";
  btn.value = label;
  btn.addEventListener("click", async (ev) => {
    ev.preventDefault();
    try {
      await onClick();
    } catch (e) {
      showMsg("admin-msg", "");
      showMsg("admin-err", e.message);
    }
  });
  form.appendChild(btn);
  return form;
}

function adminHeadCell(s) {
  const td = el("td", "auto-style10");
  td.colSpan = 2;
  const a = el("a");
  a.href = "https://" + s.domain + "/";
  a.target = "_blank";
  a.appendChild(el("strong", "", s.domain));
  td.append(a, el("span", "small", " (" + fmtSize(s.size_bytes) + ")"));
  return td;
}

function adminMetaCell(s) {
  const input = (size, value) => {
    const i = el("input");
    i.type = "text";
    i.maxLength = 120;
    i.size = size;
    i.value = value;
    return i;
  };
  const nameIn = input(15, s.name);
  const descIn = input(30, s.description);
  const td = el("td");
  const form = el("form");
  form.style.margin = "0";
  form.append(
    nameIn,
    descIn,
    actionButton("SALVA", async () => {
      await api("/api/admin/meta", {
        json: { sub: s.sub, name: nameIn.value, description: descIn.value },
      });
      showMsg("admin-msg", "info salvate!");
    }),
  );
  td.appendChild(form);
  return td;
}

function adminActionCells(s) {
  const actionsTd = el("td");
  actionsTd.append(
    actionButton("NUOVA CHIAVE", async () => {
      const out = await api("/api/admin/reset-key", { json: { sub: s.sub } });
      showMsg(
        "admin-msg",
        "NUOVA CHIAVE per " +
          out.sub +
          " (mostrata una sola volta): " +
          out.token,
      );
    }),
    actionButton("CANCELLA SITO", async () => {
      if (!confirm("Cancellare " + s.domain + " e tutti i suoi file?")) return;
      await api("/api/admin/delete", { json: { sub: s.sub } });
      await loadPanel();
    }),
  );
  return [el("td", "small", "Azioni:"), actionsTd];
}

async function loadPanel() {
  const data = await api("/api/admin/sites");
  $("admin-login-box").style.display = "none";
  $("admin-panel-box").style.display = "";

  let total = 0;
  for (const s of data.sites) total += s.size_bytes;
  $("admin-stats").textContent =
    data.sites.length +
    " siti registrati, spazio totale occupato: " +
    fmtSize(total);

  fillRows(
    $("admin-site-list"),
    data.sites,
    (s) => [
      rowOf(adminHeadCell(s)),
      rowOf([el("td", "small", "Nome / Descrizione:"), adminMetaCell(s)]),
      rowOf(adminActionCells(s)),
    ],
    "Nessun sito registrato.",
  );
}

onPage("admin", async () => {
  wireForm("admin-login-form", "admin-err", async () => {
    await api("/api/admin/login", {
      json: { token: $("admin-key").value },
    });
    await loadPanel();
  });

  $("admin-logout-btn").addEventListener("click", async () => {
    await api("/api/admin/logout", { json: {} });
    location.reload();
  });

  try {
    await loadPanel();
  } catch (e) {
    $("admin-login-box").style.display = "";
    $("admin-panel-box").style.display = "none";
  }
});
