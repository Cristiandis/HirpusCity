function $(id) {
  return document.getElementById(id);
}

async function api(path, options) {
  const opts = Object.assign({ headers: {} }, options || {});
  if (opts.json !== undefined) {
    opts.method = opts.method || "POST";
    opts.headers["Content-Type"] = "application/json";
    opts.body = JSON.stringify(opts.json);
    delete opts.json;
  }
  const res = await fetch(path, opts);
  let data = {};
  try {
    data = await res.json();
  } catch (e) {}
  if (!res.ok) {
    throw new Error(data.error || "errore " + res.status);
  }
  return data;
}

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined && text !== null) node.textContent = text;
  return node;
}

function showMsg(id, text) {
  const box = $(id);
  box.textContent = text;
  box.style.display = text ? "" : "none";
}

function fmtSize(bytes) {
  if (bytes >= 1073741824) return (bytes / 1073741824).toFixed(2) + " GB";
  if (bytes >= 1048576) return (bytes / 1048576).toFixed(1) + " MB";
  if (bytes >= 1024) return (bytes / 1024).toFixed(1) + " KB";
  return bytes + " B";
}

function rowOf(cells) {
  const tr = el("tr");
  for (const c of [].concat(cells)) tr.appendChild(c);
  return tr;
}

function siteRow(domain, name, description) {
  const td = el("td", "auto-style10");
  const link = el("a", "", name || domain);
  link.href = "https://" + domain + "/";
  link.target = "_blank";
  td.appendChild(link);
  td.appendChild(el("span", "small", " (" + domain + ")"));
  td.appendChild(el("br"));
  td.appendChild(el("span", "small", description));
  return td;
}

function fillRows(list, items, renderRows, emptyText) {
  list.textContent = "";
  if (!items.length) {
    list.appendChild(rowOf(el("td", "auto-style10 center", emptyText)));
    return;
  }
  for (const item of items) {
    for (const tr of [].concat(renderRows(item))) list.append(tr);
  }
}

async function wireForm(formId, errId, fn) {
  $(formId).addEventListener("submit", async (ev) => {
    ev.preventDefault();
    showMsg(errId, "");
    try {
      await fn(ev);
    } catch (e) {
      showMsg(errId, e.message);
    }
  });
}

function onPage(name, fn) {
  document.addEventListener("DOMContentLoaded", async () => {
    if (document.body.dataset.page === name) {
      try {
        await fn();
      } catch (e) {
        console.error(e);
      }
    }
  });
}

async function openRandomSite() {
  const sites = await api("/api/sites?limit=500");
  if (!sites.length) return;
  const s = sites[Math.floor(Math.random() * sites.length)];
  location.href = "//" + s.domain + "/";
}
