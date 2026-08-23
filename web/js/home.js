onPage("home", async () => {
  const session = await api("/api/session");
  if (session.sub) {
    $("logged-as").textContent =
      "Ciao " + session.sub + "." + session.base_domain + "!";
    $("logged-box").style.display = "";
    $("signup-box").style.display = "none";
    $("login-box").style.display = "none";
  } else {
    $("domain-suffix").textContent = "." + session.base_domain;
  }

  wireForm("signup-form", "home-err", async () => {
    const out = await api("/api/signup", {
      json: { subdomain: $("signup-sub").value },
    });
    $("created-domain").textContent = out.domain;
    $("created-token").textContent = out.token;
    $("signup-box").style.display = "none";
    $("login-box").style.display = "none";
    $("created-box").style.display = "";
  });

  wireForm("login-form", "home-err", async () => {
    await api("/api/login", {
      json: {
        subdomain: $("login-sub").value,
        token: $("login-token").value,
      },
    });
    location.href = "/dashboard.html";
  });

  const sites = await api("/api/sites?limit=10");
  fillRows(
    $("recent-list"),
    sites,
    (s) => rowOf(siteRow(s.domain, s.name, s.description)),
    "Nessun sito ancora... sii il primo!",
  );

  $("lucky-btn").addEventListener("click", () => openRandomSite());
});
