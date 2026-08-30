# Mongo host backup pipeline

Daily encrypted dump → S3 + monthly restore drill, packaged for the operator
to drop onto a host that can reach the production database.

> **The source database is no longer local to this host.** Mongo moved to
> Zeabur-managed hosting during August 2026, but `MONGO_URI` still pointed at
> `127.0.0.1:27017`. The timer kept succeeding against the drained
> self-hosted mongod, so every dump from roughly 2026-08-19 onward is of a
> near-empty database — around 6.9 KB, against a production dataset of ~8 MB
> and 32k documents. With a 30-day lifecycle on `daily/`, that means no
> usable backup of production currently exists in S3.
>
> Point `MONGO_URI` at the Zeabur connection string, and set
> `ASSERT_COLLECTION` / `MIN_ARCHIVE_BYTES` (below) so the same failure
> cannot be silent a second time.

## Files

| Path on repo | Path on host | Purpose |
| --- | --- | --- |
| `bandao-backup.sh` | `/usr/local/bin/bandao-backup.sh` | Daily dump + age + s3 cp, with pre-dump and size guards |
| `bandao-restore-drill.sh` | `/usr/local/bin/bandao-restore-drill.sh` | Pulls latest dump, restores to scratch DB, asserts counts |
| `bandao-backup.service` | `/etc/systemd/system/bandao-backup.service` | One-shot unit invoked by the timer |
| `bandao-backup.timer` | `/etc/systemd/system/bandao-backup.timer` | Daily 03:30 schedule |

## One-time setup on the backup host

Any always-on Linux host that can reach the production Mongo will do; it no
longer has to be the database host itself. Assumes Debian 12 / Ubuntu 22.04+.
Adjust package names for other distros.

```bash
# Tooling — mongo client, awscli v2, age.
sudo apt-get update
sudo apt-get install -y mongodb-mongosh mongodb-database-tools awscli age
```

Generate the encryption keypair on the operator's workstation (NOT on the
backup host) and copy only the public key to the host:

```bash
# on workstation:
age-keygen -o bandao-backup.key
# stash bandao-backup.key in the operator's password manager (off-host).
# the public key is the line `# public key: age1xxxxxxxx...` — note it.
```

Drop the script files in place:

```bash
sudo install -m 0755 bandao-backup.sh /usr/local/bin/
sudo install -m 0755 bandao-restore-drill.sh /usr/local/bin/
sudo install -m 0644 bandao-backup.service /etc/systemd/system/
sudo install -m 0644 bandao-backup.timer /etc/systemd/system/
```

Create `/etc/bandao-backup.env` (mode 0600, root-owned):

```
# Must reach the database production actually writes to. Since the Zeabur
# migration this is NOT 127.0.0.1 — use the Zeabur connection string.
MONGO_URI=mongodb://backup_user:<pw>@<zeabur-host>:<port>/?authSource=admin
MONGO_DB=bandao
AGE_RECIPIENT=age1xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
S3_BUCKET=backup.ccmos.tw
S3_REGION=ap-northeast-1
S3_ACCESS_KEY_ID=AKIAxxxxxxxxxxxxxxxx
S3_SECRET_ACCESS_KEY=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
# optional:
# S3_PREFIX=bandao

# Guards — both optional, both strongly recommended. See "Guards" below.
ASSERT_COLLECTION=checkin_events
ASSERT_MIN_COUNT=1000
MIN_ARCHIVE_BYTES=100000
```

Lock it down:

```bash
sudo chmod 0600 /etc/bandao-backup.env
sudo chown root:root /etc/bandao-backup.env
```

The bucket is `backup.ccmos.tw` in AWS account `869847891424`, under the
`daily/` prefix. (Earlier revisions of this file named a
`bandao-mongo-backups-apne1` bucket that does not exist in any of the
project's accounts.)

### Guards

Every command in the old pipeline exited 0 while it backed up the wrong
database for weeks, because a dump of an empty database is a valid dump.
Two opt-in checks close that gap; set both.

| Key | Effect |
| --- | --- |
| `ASSERT_COLLECTION` | Counts documents in this collection (via `mongosh`) **before** dumping. Nothing runs if the source does not look like production. |
| `ASSERT_MIN_COUNT` | Minimum that count must reach. Default `1`; set it near the real row count, not to 1 — the failure being guarded against left a handful of documents behind, not zero. |
| `MIN_ARCHIVE_BYTES` | Refuses to upload an archive smaller than this. Default `100000`. |

Exit codes: `64` missing config, `65` pre-dump assertion failed, `66`
archive below the size floor. A non-zero exit fails the systemd unit, so
these surface wherever the timer's failures are already routed.

The script stages the archive to a temp file (removed on exit) instead of
streaming into `aws s3 cp -`, because a stream cannot be measured before it
lands. Scratch space needed is one compressed dump — single-digit MB today.

Mongo user for backups (run from `mongosh` as a dbAdmin):

```javascript
use admin
db.createUser({
  user: "backup_user",
  pwd: "<strong password>",
  roles: [ { role: "backup", db: "admin" } ]
})
```

Enable the timer:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now bandao-backup.timer
sudo systemctl list-timers | grep bandao
```

Trigger the first run manually and watch the journal:

```bash
sudo systemctl start bandao-backup.service
journalctl -u bandao-backup.service -f
```

Verify the object landed:

```bash
aws s3 ls s3://$S3_BUCKET/daily/
```

## Monthly restore drill

The drill must NOT keep the private key on the host. Mount it just for the
run, then remove:

```bash
# Pull the encrypted private key from the operator's secret store into a
# tmpfs path:
sudo mkdir -p /run/bandao-drill
sudo mount -t tmpfs -o size=16k,mode=0700 tmpfs /run/bandao-drill
# (paste / scp the key into /run/bandao-drill/age.key with mode 0400)

sudo env \
  AGE_IDENTITY_FILE=/run/bandao-drill/age.key \
  ASSERT_COLLECTION=checkin_events \
  ASSERT_MIN_COUNT=1000 \
  /usr/local/bin/bandao-restore-drill.sh

# Always tear down:
sudo umount /run/bandao-drill
sudo rmdir /run/bandao-drill
```

Set `ASSERT_MIN_COUNT` to something near the real row count rather than `1`.
A drill that only asserts "at least one document" passes against a dump of a
drained database, which is exactly the state that went unnoticed for weeks.

A failed drill exits non-zero and writes the failure to syslog tagged
`bandao-restore-drill`. Wire that into the operator's alerting (mailx,
ntfy, Sentry, etc.) — implementation depends on the host's existing
notification stack and is out of scope for this change.

## S3 bucket lifecycle

Apply this lifecycle JSON to the bucket so daily/weekly/monthly retention
is enforced by S3, not by the script:

```json
{
  "Rules": [
    {
      "ID": "expire-daily-30d",
      "Filter": { "Prefix": "daily/" },
      "Status": "Enabled",
      "Expiration": { "Days": 30 }
    }
  ]
}
```

Promotion to weekly / monthly snapshots is a separate pipeline (e.g. an S3
Replication rule into `weekly/` and `monthly/` prefixes, or a cron on the
host that copies the most recent daily into those prefixes on Sundays / the
1st of the month). The simplest first iteration is to keep `daily/` only
and revisit promotion once the basics are stable.
