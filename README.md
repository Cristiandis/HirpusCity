# HIRPUSPAGES (hirpuscity)

A 100% offline, self-hosted Neocities clone: point any domain you control at
it and every user gets their own subdomain.
Users pick a subdomain (`name.pages.hirpus`), receive a **UUID key** as their
credential, upload `.html` / `.css` / image files and their site goes online.

- Backend: Rust + axum, **no database**, just a single JSON registry
  (`sites.json`: key, name, description per subdomain)
- Quota: **1 GB per site** (configurable)
- Purely **static frontend** (`web/`): retro HTML pages talking to a small
  JSON API (`/api/*`) via vanilla JavaScript: no frameworks, no build step
- Works behind any external reverse proxy with a wildcard TLS certificate

## Installation

```
git clone https://github.com/Cristiandis/HirpusCity && cd HirpusCity/
docker compose up -d
```

Pulls the published image from `ghcr.io/cristiandis/hirpuscity:latest`
(multi-arch: amd64 + arm64). The image is rebuilt automatically by the
GitHub Actions workflow in `.github/workflows/` on every push to `main`.
To build locally instead, uncomment `build: .` in docker-compose.yml and
run `docker compose up -d --build`.

Data lives in `./data` (volume); the service listens on `127.0.0.1:8080`
for your reverse proxy. To update: `docker compose pull && docker compose up -d`.

## Configuration

| Variable               | Default          |
| ---------------------- | ---------------- |
| `HIRPUS_LISTEN`        | `127.0.0.1:8080` |
| `HIRPUS_DATA_DIR`      | `./data`         |
| `HIRPUS_ASSETS_DIR`    | `./web`          |
| `HIRPUS_BASE_DOMAIN`   | `pages.hirpus`   |
| `HIRPUS_SITE_QUOTA_MB` | `1024`           |
| `HIRPUS_ADMIN_UUID`    | _(unset)_        |

## Admin panel

Set `HIRPUS_ADMIN_UUID` to any UUID (e.g. from `uuidgen`) to enable the
admin panel at **`/admin.html`**: lists all sites with disk usage, edits
name/description, regenerates a user's key (**NUOVA CHIAVE**) and deletes
sites (**CANCELLA SITO**). If the variable is unset or not a valid UUID,
the admin routes don't exist at all.

## Setup

1. **DNS**: point `pages.hirpus` and `*.pages.hirpus` at the server
   (A record + wildcard record). On a plain LAN or air-gapped network,
   any local DNS server (dnsmasq, router) does the job.

2. **TLS**: you need a wildcard certificate for the chosen domain
   (`pages.hirpus` + `*.pages.hirpus`), issued by any CA clients
   trust, be it public (e.g. Let's Encrypt) or your own for private networks.

3. **Reverse proxy**: route both hosts to `http://127.0.0.1:8080`. Whatever
   software you use, three requirements:
   - two rules: `pages.hirpus` → platform UI, `*.pages.hirpus` → user sites;
   - **pass the Host header unchanged**: that's how the backend picks the
     site to serve (nginx: `proxy_set_header Host $host;`);
   - body size limit above the quota (nginx: `client_max_body_size`).

   Minimal nginx example:

   ```nginx
   server {
       listen 443 ssl;
       server_name pages.hirpus *.pages.hirpus;
       ssl_certificate     /etc/nginx/certs/pages.pem;
       ssl_certificate_key /etc/nginx/certs/pages-key.pem;
       client_max_body_size 1100M;

       location / {
           proxy_pass http://127.0.0.1:8080;
           proxy_set_header Host $host;
       }
   }
   ```

## How it works

- Signup: pick a subdomain on the homepage → the API returns your UUID key,
  shown once. The key is the whole credential.
- Login: subdomain + key set an `hcity=<sub>:<key>` cookie, verified against
  the registry on every API call.
- Dashboard: multi-file upload, file deletion, site name/description (used
  by search), quota bar, visit stats, and self-service site
  deletion (danger zone).
- Search: every word must match (AND), ranked by match quality (exact name,
  then name, description, domain) and recency; matched words are highlighted
  in the results (HIRPUSEARCH).
- Analytics: each served request on a user site increments its visit counter
  (shown on the dashboard, total in the admin panel). Stored in the registry.
- User sites are served statically based on the Host header, `index.html`
  as directory entry, optional custom `404.html`.
- Frontend lives in `web/` (HTML pages, `style.css`, `search.css` for the
  Google-style search pages, one JS module per page in `js/`, plus
  `starter_index.html` for new sites): everything there is served
  automatically and read from disk on each request, so edits are live
  immediately and new pages need no code changes.
- Allowed uploads: `html htm css png jpg jpeg gif webp svg ico` (no JS),
  all counted against the quota.

## License

MIT. See [LICENSE](LICENSE).
