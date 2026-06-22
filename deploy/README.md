# EC2 deployment guide

Step-by-step from a fresh AWS account to a working HTTPS deployment.

**Target shape:** one EC2 instance running a single Docker image (Caddy + Rust API + built SPA) plus sibling Postgres and Redis containers. The image is **built on a dev machine (or CI), pushed to a container registry, and pulled by the EC2** — the EC2 itself never compiles code.

```
   dev/CI ──build──► registry (GHCR / ECR / Docker Hub)
                         │
                         │ docker compose pull
                         ▼
   internet ──443──► EC2:Caddy ─┬─►  /         ──► /srv/dist  (SPA)
                                └─►  /v1/*     ──► api:8080   (REST + WS)
                                      │
                                ┌─────┼─────┐
                                ▼     ▼     ▼
                            postgres redis  (compose siblings)
```

**Why pull, not build?** The Rust release build needs ~2 GB RAM and ~5 min. Pulling a pre-built image lets the EC2 stay small (t3.small / t4g.small), keeps deploys to ~30 seconds, and makes rollbacks trivial (`APP_IMAGE=...:v1.2.3` in `.env`).

---

## Before you begin — checklist

- [ ] AWS account with permission to create EC2 + EBS + Elastic IPs
- [ ] A domain name you control (e.g. `interview.example.com`) — required for Let's Encrypt
- [ ] A container registry to push to:
  - **GHCR** (default in this guide) — free for private images on GitHub, simplest auth
  - **ECR** — AWS-native, IAM-friendly, no external dep
  - **Docker Hub** — simple but has tighter rate limits on free tier
- [ ] An [Anthropic API key](https://console.anthropic.com/) with budget for Sonnet 4.6 + Haiku 4.5
- [ ] An [AssemblyAI API key](https://www.assemblyai.com/) for streaming speech-to-text — **required**; the API process refuses to start without it
- [ ] A [Resend API key](https://resend.com/) for HR report emails
- [ ] A dev machine with Docker installed (Linux, Mac, or WSL2)

Estimated time: **45–60 minutes** for first deploy. Subsequent updates are `build + push + ssh + pull + up` (~3–5 min total).

---

## Step 1 — Set up the container registry

### Option A — GHCR (recommended)

GHCR uses your GitHub account; no separate signup.

1. Create a **Personal Access Token (classic)** at <https://github.com/settings/tokens>
   - Scope: `write:packages`, `read:packages`, `delete:packages`
   - Save the token — you'll need it twice (once on dev to push, once on EC2 to pull)
2. Choose an image name: `ghcr.io/<your-gh-username>/mock-interview-app`
3. Log in on your dev machine:
   ```bash
   echo "$GHCR_PAT" | docker login ghcr.io -u <your-gh-username> --password-stdin
   ```

### Option B — ECR

```bash
# Create the repo
aws ecr create-repository --repository-name mock-interview-app --region us-east-1

# Log in
aws ecr get-login-password --region us-east-1 | \
  docker login --username AWS --password-stdin <account>.dkr.ecr.us-east-1.amazonaws.com
```

### Option C — Docker Hub

1. Create a repo at <https://hub.docker.com/repositories> (private if you want)
2. Generate an access token at <https://hub.docker.com/settings/security>
3. Log in: `echo "$TOKEN" | docker login -u <username> --password-stdin`

---

## Step 2 — Build and push the image (from your dev machine)

From the repo root:

```bash
# Tag the image with your registry path
IMAGE=ghcr.io/<your-gh-username>/mock-interview-app:latest

# Build for linux/amd64 even on Apple Silicon — EC2 standard instances are amd64.
docker buildx build --platform linux/amd64 -t "$IMAGE" .

# Push
docker push "$IMAGE"
```

First build: **5–10 min** (Rust workspace + npm). Subsequent builds reuse the cargo cache and finish in ~1–2 min.

**Tag a version on releases** so you can roll back cleanly:

```bash
docker tag "$IMAGE" "ghcr.io/<gh-user>/mock-interview-app:v1.2.3"
docker push "ghcr.io/<gh-user>/mock-interview-app:v1.2.3"
```

> A GitHub Actions workflow that does this on every push to `main` is below in [Automate with CI](#optional--automate-build--push-with-github-actions).

---

## Step 3 — Launch the EC2 instance

Open the EC2 console → **Launch instance**.

| Setting | Value |
|---|---|
| Name | `mock-interview-prod` |
| AMI | **Amazon Linux 2023** (or Ubuntu 22.04) |
| Architecture | `64-bit (x86)` |
| Instance type | **`t3.small`** is enough — we only pull and run, no build |
| Key pair | Create or pick one you have the `.pem` for |
| Storage | **20 GB gp3** (retention worker keeps Postgres bounded) |

### Security group

| Type | Port | Source | Why |
|---|---|---|---|
| SSH | 22 | Your IP (`x.x.x.x/32`) | Admin |
| HTTP | 80 | `0.0.0.0/0` | Let's Encrypt HTTP-01 ACME challenge |
| HTTPS | 443 | `0.0.0.0/0` | App traffic |

Postgres (5432) and Redis (6379) are **not** exposed — they live on the compose internal network only.

### Elastic IP

Allocate one and associate it with the instance. This pins the public IP so DNS doesn't break on reboot.

---

## Step 4 — Point DNS at the instance

In your DNS provider, add an `A` record:

```
Type:  A
Name:  interview.example.com
Value: <your Elastic IP>
TTL:   300
```

Verify it resolves **before** continuing — Let's Encrypt rate-limits failed ACME attempts:

```bash
dig +short interview.example.com
# should print your Elastic IP
```

---

## Step 5 — SSH in and install Docker

```bash
ssh -i ~/path/to/key.pem ec2-user@interview.example.com
```

**Amazon Linux 2023:**

```bash
sudo dnf update -y
sudo dnf install -y docker git
sudo systemctl enable --now docker
sudo usermod -aG docker ec2-user

# Compose v2 plugin
sudo mkdir -p /usr/local/lib/docker/cli-plugins
sudo curl -fsSL "https://github.com/docker/compose/releases/latest/download/docker-compose-linux-$(uname -m)" \
     -o /usr/local/lib/docker/cli-plugins/docker-compose
sudo chmod +x /usr/local/lib/docker/cli-plugins/docker-compose

exit  # log out + back in so the docker group takes effect
```

**Ubuntu 22.04:**

```bash
sudo apt-get update
sudo apt-get install -y docker.io docker-compose-plugin git
sudo systemctl enable --now docker
sudo usermod -aG docker ubuntu
exit
```

SSH back in and verify:

```bash
docker --version
docker compose version
```

---

## Step 6 — Authenticate Docker to the registry (on the EC2)

The EC2 needs to be able to `docker pull` your image.

### Option A — GHCR

```bash
echo "$GHCR_PAT" | docker login ghcr.io -u <your-gh-username> --password-stdin
```

Stores credentials at `~/.docker/config.json`. Compose `pull` will use them automatically.

### Option B — ECR

The clean AWS-native way is to attach an **IAM role** to the EC2 with `AmazonEC2ContainerRegistryReadOnly`, then install the ECR credential helper so docker pulls "just work":

```bash
sudo dnf install -y amazon-ecr-credential-helper
mkdir -p ~/.docker
cat > ~/.docker/config.json <<'EOF'
{"credHelpers": {"<account>.dkr.ecr.us-east-1.amazonaws.com": "ecr-login"}}
EOF
```

No tokens to rotate.

### Option C — Docker Hub

```bash
echo "$DOCKERHUB_TOKEN" | docker login -u <username> --password-stdin
```

---

## Step 7 — Drop the compose file + .env on the EC2

You only need two files from the repo on the EC2 — `deploy/docker-compose.yml` and `deploy/.env`. Easiest path is a sparse clone:

```bash
git clone --depth=1 --filter=blob:none --sparse <your-repo-url> mock-interview-ai
cd mock-interview-ai
git sparse-checkout set deploy
```

Or just `scp` the two files over from your dev machine:

```bash
# from your dev machine
scp -i ~/path/to/key.pem deploy/docker-compose.yml deploy/.env.example \
    ec2-user@interview.example.com:~/
```

Either way you should end up with `docker-compose.yml` and `.env.example` accessible on the EC2.

---

## Step 8 — Generate secrets

You need four strong secrets. Generate them anywhere (your dev machine or the EC2) and save somewhere secure:

```bash
# JWT signing secret (≥ 32 bytes)
openssl rand -base64 48

# Transcript hash-chain key (exactly 32 bytes as hex)
openssl rand -hex 32

# Postgres password (≥ 24 chars, no slashes)
openssl rand -base64 24 | tr -d '/+='
```

---

## Step 9 — Configure `.env` on the EC2

```bash
cp deploy/.env.example deploy/.env
chmod 600 deploy/.env
$EDITOR deploy/.env
```

Fill in **every value marked `replace-me-…`**. Most important:

| Variable | What to put |
|---|---|
| `APP_IMAGE` | Full registry path, e.g. `ghcr.io/your-gh-user/mock-interview-app:v1.2.3` (use a tag, not `:latest`, for repeatable rollouts) |
| `DOMAIN` | `interview.example.com` |
| `ACME_EMAIL` | An email Let's Encrypt can reach you at |
| `POSTGRES_PASSWORD` | The Postgres password you generated |
| `DATABASE_URL` | Update password portion: `postgres://mockinterview:<password>@postgres:5432/mockinterview` |
| `WEB_BASE_URL` | `https://interview.example.com` (must match `DOMAIN`) |
| `CORS_ORIGINS` | `https://interview.example.com` (must match `DOMAIN`) |
| `ANTHROPIC_API_KEY` | From the Anthropic console |
| `ASSEMBLYAI_API_KEY` | From the AssemblyAI console — required; the API won't boot without it |
| `RESEND_API_KEY` | From the Resend console |
| `MAIL_FROM_ADDRESS` | A verified sender on your Resend domain |
| `JWT_SIGNING_SECRET` | The `openssl rand -base64 48` value |
| `TRANSCRIPT_SIGNING_SECRET_HEX` | The `openssl rand -hex 32` value |

**Decide retention windows now.** The `RETENTION_*_DAYS` block controls how long raw interview data stays in Postgres before the hourly worker purges it. Defaults: 7 days for transcripts, 30 days for everything else. **Shrinking these later triggers a large delete on the next tick.**

---

## Step 10 — Pull and start

```bash
docker compose -f deploy/docker-compose.yml pull
docker compose -f deploy/docker-compose.yml up -d
```

The `pull` only fetches the `app` image — Postgres and Redis come from Docker Hub (pre-cached or pulled here). First `up`: ~30 seconds.

---

## Step 11 — Verify

```bash
docker compose -f deploy/docker-compose.yml ps
```

All three services (`app`, `postgres`, `redis`) should be `running (healthy)`.

```bash
docker compose -f deploy/docker-compose.yml logs app | grep -E "certificate|migrations|retention"
```

You should see:
- Caddy: `certificate obtained successfully` for your domain
- API: `migrations applied`
- API: `retention worker started` (with the configured day windows)

If Caddy logs `unable to get certificate`:
1. DNS isn't propagated yet (`dig +short $DOMAIN` doesn't return your IP)
2. Security group is blocking port 80 inbound
3. Let's Encrypt rate-limited you after earlier failed attempts — wait an hour

---

## Step 12 — First login

Open `https://interview.example.com` in any modern browser. Speech-to-text now runs **server-side** (the browser captures and streams PCM audio over the interview WebSocket; the API proxies it to AssemblyAI), so the candidate's browser only needs microphone access, WebSocket, and AudioWorklet support — no reliance on the flaky Web Speech API. Microphone access requires the HTTPS origin you just set up.

The first admin account is bootstrapped by migration `0002_bootstrap_default_account.sql`. Sign in, then go to **Invites** to create accounts for the rest of the team.

---

## Step 13 — Set up automated backups (recommended)

Two volumes are worth backing up:

| Volume | Why |
|---|---|
| `mock-interview_pgdata` | All application data |
| `mock-interview_caddy_data` | Let's Encrypt account + certs (avoids ACME rate limits on restore) |

Save as `/usr/local/bin/backup-app.sh`:

```bash
#!/bin/bash
set -e
BACKUP_DIR=/var/backups/mock-interview
DATE=$(date +%F-%H%M)

mkdir -p "$BACKUP_DIR"
for vol in mock-interview_pgdata mock-interview_caddy_data; do
    docker run --rm -v "$vol:/data" -v "$BACKUP_DIR:/backup" alpine \
        tar czf "/backup/$vol-$DATE.tar.gz" -C /data .
done

# Optional: ship to S3 and prune locally
# aws s3 sync "$BACKUP_DIR" "s3://your-backup-bucket" --delete
# find "$BACKUP_DIR" -mtime +7 -delete
```

Then `sudo crontab -e`:

```cron
0 3 * * * /usr/local/bin/backup-app.sh
```

---

## The standard deploy loop (after first setup)

Once you're set up, releasing a new version is:

```bash
# On your dev machine — build a new tagged image and push
IMAGE=ghcr.io/<gh-user>/mock-interview-app:v1.2.4
docker buildx build --platform linux/amd64 -t "$IMAGE" .
docker push "$IMAGE"

# On the EC2 — bump the tag and roll
ssh ec2-user@interview.example.com
sed -i 's|^APP_IMAGE=.*|APP_IMAGE=ghcr.io/<gh-user>/mock-interview-app:v1.2.4|' deploy/.env
docker compose -f deploy/docker-compose.yml pull
docker compose -f deploy/docker-compose.yml up -d
```

To **roll back**: change `APP_IMAGE` to the previous tag, `pull`, `up -d`. Postgres data and Let's Encrypt certs survive because they're in named volumes, not the image.

---

## Optional — automate build + push with GitHub Actions

Create `.github/workflows/build-image.yml`:

```yaml
name: build-image

on:
  push:
    branches: [main]
    tags: ['v*']

permissions:
  contents: read
  packages: write

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: docker/setup-buildx-action@v3
      - uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - uses: docker/metadata-action@v5
        id: meta
        with:
          images: ghcr.io/${{ github.repository_owner }}/mock-interview-app
          tags: |
            type=ref,event=branch
            type=semver,pattern={{version}}
            type=sha,prefix=sha-
      - uses: docker/build-push-action@v6
        with:
          context: .
          platforms: linux/amd64
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

Every push to `main` produces `:main` and `:sha-<hash>` tags. Every `vX.Y.Z` tag produces `:X.Y.Z`. Build cache is shared across runs via GitHub Actions cache, so subsequent builds finish in 1–2 minutes.

---

## Day-2 operations

### Tail logs

```bash
docker compose -f deploy/docker-compose.yml logs -f app
docker compose -f deploy/docker-compose.yml logs -f postgres
```

### Apply an `.env` change

```bash
$EDITOR deploy/.env
docker compose -f deploy/docker-compose.yml up -d
```

### Inspect retention worker metrics

```bash
docker compose -f deploy/docker-compose.yml exec app \
    sh -c 'apt-get install -y curl >/dev/null 2>&1; curl -s 127.0.0.1:9090/metrics | grep retention_'
```

### Postgres shell

```bash
docker compose -f deploy/docker-compose.yml exec postgres \
    psql -U mockinterview -d mockinterview
```

### Stop everything (keeps data)

```bash
docker compose -f deploy/docker-compose.yml down
```

### Stop everything and nuke data (DANGEROUS)

```bash
docker compose -f deploy/docker-compose.yml down -v
```

---

## Holding raw interview data for an audit

The retention worker purges transcripts / answers / PII on its hourly tick. For a one-off forensic export, disable it before the tick:

```bash
# 1. Disable
sed -i 's/^FEATURE_RETENTION_WORKER=.*/FEATURE_RETENTION_WORKER=false/' deploy/.env
docker compose -f deploy/docker-compose.yml up -d app

# 2. Do the export from the postgres container

# 3. Re-enable
sed -i 's/^FEATURE_RETENTION_WORKER=.*/FEATURE_RETENTION_WORKER=true/' deploy/.env
docker compose -f deploy/docker-compose.yml up -d app
```

Do not edit the worker source.

---

## Air-gapped or no-registry fallback

If you can't (or don't want to) use a registry, save the image to a tarball and `ssh | docker load`:

```bash
# Dev machine
docker buildx build --platform linux/amd64 -t mock-interview-app:latest .
docker save mock-interview-app:latest | gzip | \
    ssh ec2-user@interview.example.com 'gunzip | docker load'

# On the EC2 — set APP_IMAGE to the local tag and PULL_POLICY=never
sed -i 's|^APP_IMAGE=.*|APP_IMAGE=mock-interview-app:latest|' deploy/.env
sed -i 's|^PULL_POLICY=.*|PULL_POLICY=never|' deploy/.env
docker compose -f deploy/docker-compose.yml up -d
```

---

## Troubleshooting

**`docker compose pull` returns `unauthorized`.** Your registry login expired or scope is wrong. Re-run the login step (Step 6). For GHCR, the PAT needs `read:packages`.

**`docker compose pull` returns `manifest unknown`.** The tag in `APP_IMAGE` doesn't exist in the registry. Confirm the tag with `docker manifest inspect $APP_IMAGE` or browse the registry UI.

**Caddy can't get a cert.** `docker compose logs app | grep -i cert`. Most common: DNS isn't pointing at the EC2 (`dig +short $DOMAIN`), port 80 blocked inbound, or Let's Encrypt rate-limited you. Wait an hour after fixing.

**`migrations applied` never appears.** Postgres password mismatch between `POSTGRES_PASSWORD` and the password in `DATABASE_URL`. Tear down and start fresh: `docker compose down -v` (nukes pgdata — only safe before first real use).

**SPA loads but every API call returns 502.** The `api` process isn't listening inside the container. Check `docker compose logs app | tail -50` for a Rust panic. Common cause: a malformed env var (e.g., `JWT_SIGNING_SECRET` < 32 bytes).

**WebSocket connection fails.** Route is `/v1/ws/interview` and goes through the same `/v1/*` reverse-proxy block in the Caddyfile. Check browser devtools → Network → WS for the close code, then `docker compose logs app` for matching errors.

---

## Production checklist (dev → prod deltas)

The committed `backend/.env.backend.example` and `frontend/example.env` are **dev defaults**. `deploy/.env.example` is the prod template — everything below is already set in it.

| Setting | Dev | Prod |
|---|---|---|
| `CORS_ORIGINS` | `http://localhost:4173,...` (incl. LAN IPs) | `https://$DOMAIN` only |
| `WEB_BASE_URL` | `http://localhost:4173` | `https://$DOMAIN` |
| `COOKIES_SECURE` | `false` | **`true`** (Caddy terminates TLS) |
| `FEATURE_RATE_LIMIT` | `false` | **`true`** |
| `METRICS_LISTEN_ADDR` | `0.0.0.0:9090` | `127.0.0.1:9090` (internal only) |
| `LISTEN_ADDR` | `0.0.0.0:8080` | `0.0.0.0:8080` (compose network internal) |
| `JWT_SIGNING_SECRET` | hardcoded dev string | new value via `openssl rand -base64 48` |
| `TRANSCRIPT_SIGNING_SECRET_HEX` | hardcoded dev hex | new value via `openssl rand -hex 32` |
| `HR_DEV_TOKEN` | optional dev fallback | **must be unset** |
| `FEATURE_REDIS_OUTBOX/NONCES/WS_LOCKS` | optional | `true` (Redis sibling exists) |
| `ANTHROPIC_MODEL_PRIMING/SCORING/FOLLOWUP` | varies | `claude-sonnet-4-6` for quality |
| `ANTHROPIC_MODEL_GRADING` | varies | `claude-haiku-4-5-20251001` (cost-sensitive hot path) |
| `TRANSCRIPT_KEY_ID` | `dev-v1` | `prod-v1` |
| `JWT_KEY_ID` | `v1` | `prod-v1` |
| `FEATURE_RETENTION_WORKER` | `true` (default) | `true` — tune day windows before first boot |
| `FEATURE_OTEL` | varies | `false` unless you have an OTLP collector |

**Critical secrets to rotate before first prod boot:**
- `ANTHROPIC_API_KEY` — the dev `.env` may contain a real key; rotate it via the Anthropic console.
- `JWT_SIGNING_SECRET` — never reuse the dev value.
- `TRANSCRIPT_SIGNING_SECRET_HEX` — never reuse the dev value (transcript chain integrity depends on it).
- `POSTGRES_PASSWORD` — never reuse the dev `mockinterview/mockinterview` default.
