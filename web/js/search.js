function highlightQuery(text, q) {
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return [document.createTextNode(text)];
  const lower = text.toLowerCase();
  const nodes = [];
  let pos = 0;
  while (pos < text.length) {
    let hit = -1;
    for (const w of words) {
      const i = lower.indexOf(w, pos);
      if (i >= 0 && (hit === -1 || i < hit)) hit = i;
    }
    if (hit === -1) {
      nodes.push(document.createTextNode(text.slice(pos)));
      break;
    }
    if (hit > pos) nodes.push(document.createTextNode(text.slice(pos, hit)));
    const word = words.find((w) => lower.indexOf(w, hit) === hit);
    nodes.push(el("b", "", text.slice(hit, hit + word.length)));
    pos = hit + word.length;
  }
  return nodes;
}

function resultNode(s, q) {
  const r = el("div", "g-r");

  const h = el("h3");
  const link = el("a", "", s.name || s.domain.split(".")[0]);
  link.href = "//" + s.domain + "/";
  link.target = "_blank";
  h.append(link);
  r.append(h);

  const url = el("div", "g-u", s.domain);
  r.append(url);

  if (s.description) {
    const sn = el("div", "g-s");
    for (const node of highlightQuery(s.description, q)) sn.append(node);
    r.append(sn);
  }
  return r;
}

function noResultNode(q) {
  const box = el("div", "g-none");
  const p = el("p");
  p.append(
    el("b", "", "La tua ricerca"),
    " - \u00ab" + q + "\u00bb - non ha prodotto risultati.",
  );
  box.append(p);
  box.append(el("p", "", "Suggerimenti:"));
  const ul = el("ul");
  for (const tip of [
    "Controlla che tutte le parole siano digitate correttamente.",
    "Prova con parole chiave diverse.",
    "Prova con parole pi\u00f9 generiche.",
  ]) {
    ul.append(el("li", "", tip));
  }
  box.append(ul);
  return box;
}

onPage("search", async () => {
  const q = (new URLSearchParams(location.search).get("q") || "").trim();

  if (!q) {
    $("lucky-btn").addEventListener("click", () => openRandomSite());
    return;
  }
  $("hero-mode").style.display = "none";
  $("bar-mode").style.display = "";
  $("gq").value = q;
  document.title = q + " - HIRPUSEARCH";

  const t0 = performance.now();
  let sites;
  try {
    sites = await api("/api/sites?q=" + encodeURIComponent(q));
  } catch (e) {
    $("gcount").textContent = e.message;
    return;
  }
  const secs = ((performance.now() - t0) / 1000).toFixed(2);
  const n = sites.length;
  $("gcount").textContent =
    "Circa " +
    n.toLocaleString("it-IT") +
    (n === 1 ? " risultato" : " risultati") +
    " (" +
    secs +
    " secondi)";

  const box = $("gresults");
  if (!n) {
    box.append(noResultNode(q));
    return;
  }
  for (const s of sites) box.append(resultNode(s, q));
});
