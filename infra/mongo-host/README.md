# Mongo host backup pipeline

Daily encrypted dump → S3 + monthly restore drill.

> **"Mongo host" is the production node itself** — the same box that runs
> the `api` and `admin-web` pods and `mongod`. There is no separate backup
> host and there never was; two
> earlier revisions of this README were written as though one existed, and
> a third recorded it as unlocatable. It was located on 2026-08-31 from S3
> server access logs: the `bandao-backup` requester, writing from the
> production node's own address at 03:33 Taipei — the 03:30 timer. Use
> those logs, not CloudTrail, to attribute S3 activity here; this account
> records no S3 data events.
>
> **Repaired 2026-08-31, and verified.** For months the pipeline dumped the
> drained pre-migration database — ~88 documents against production's
> ~32.4k, hence ~6.9 KB archives — while every command exited 0. Two faults
> were stacked: the wrong `MONGO_URI`, and a deployed `bandao-backup.sh`
> that predated the guards below, so nothing rejected the undersized result.
> Fixing either alone would have left the other armed.
>
> A correct archive is **~1.1 MB**, and the restore drill now passes
> (`32512 document(s) restored successfully, 0 failures`). Run it monthly:
> it is the only step that proves the archive reads back, and everything
> before it can pass on an archive nobody can restore.

## Files

| Path on repo | Path on host | Purpose |
| --- | --- | --- |
| `bandao-backup.sh` | `/usr/local/bin/bandao-backup.sh` | Daily dump + age + s3 cp, with pre-dump and size guards |
| `bandao-restore-drill.sh` | `/usr/local/bin/bandao-restore-drill.sh` | Pulls latest dump, restores to scratch DB, asserts counts |
| `bandao-backup.service` | `/etc/systemd/system/bandao-backup.service` | One-shot unit invoked by the timer |
| `bandao-backup.timer` | `/etc/systemd/system/bandao-backup.timer` | Daily 03:30 schedule |

## One-time setup

Runs on the production node. Assumes Debian 12 / Ubuntu 22.04+; adjust
package names for other distros.

```bash
# Tooling — mongo client, awscli v2, age.
sudo apt-get update
sudo apt-get install -y mongodb-mongosh mongodb-database-tools awscli age
```

Generate the encryption keypair on the operator's workstation (NOT on the
node) and copy only the public key to the node:

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
# Must reach the database production actually writes to. Loopback is correct
# again since the cutover back to a host mongod — but this exact string was
# ALSO what the broken pipeline used, back when it resolved to a drained
# mongod on the same address. Never verify this line by reading it; verify it
# with the document count the guards below assert.
MONGO_URI=mongodb://backup_user:<pw>@127.0.0.1:27017/?authSource=admin
MONGO_DB=bandao
AGE_RECIPIENT=age1xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
S3_BUCKET=<shared ccmos backup bucket>
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

The bucket name and AWS account are **not recorded here — this repository is
public.** They live in the operator's password manager alongside the IAM
credentials; a bucket name is globally unique and directly probeable, and an
account id is the starting point for cross-account enumeration. The bucket
sits in `ap-northeast-1` under the `daily/` prefix, created with versioning,
SSE-S3, Block Public Access, and the lifecycle rule below.

It is **shared with other ccmos projects**, and that is a deliberate
decision rather than an oversight. Sharing once made "is this object ours?"
unanswerable — the archives are age-encrypted and the private key is off-host
by design, so nothing about an object identifies its source. What actually
answered it was **S3 server access logging**, which records the requester and
source IP of every `PUT`. A dedicated bucket infers provenance from where an
object landed; the access log states it, and covers the other projects in the
bucket too.

So the three controls that matter here are the access log (provenance), the
size guard below (was the right database dumped), and the restore drill (does
it come back). Bucket isolation was a proxy for the first and buys nothing
once real logging exists. If you ever turn access logging off, revisit this.

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

Mongo user for backups (run from `mongosh` as `root`):

```javascript
use admin
db.createUser({
  user: "backup_user",
  pwd: "<strong password>",
  roles: [ { role: "backup", db: "admin" } ]
})
```

**`backup` only — do not add `restore`.** The user found on the box in
August 2026 carried both, which this README never documented. `restore` is
not a harmless complement to `backup`: it grants `createUser` and
`createRole` on `admin`, so a credential kept in plaintext in
`/etc/bandao-backup.env` — beside the S3 keys, on a host that is also
reachable over the tailnet — could promote itself to `root`. Restores and
drills run as `root`, which is an operator-driven procedure, not a
scheduled one.

Generate the password with `openssl rand -hex 32`, not `-base64`: base64
emits `/` and `+`, which have to be percent-encoded inside a connection
string and are easy to get silently wrong.

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

The drill needs two things the daily backup does not, and both must live on
a tmpfs that is torn down afterwards: the `age` **private key**, and a Mongo
credential that can **write**. `backup_user` holds `backup` only — it reads.
`mongorestore` creating the scratch database fails as `(Unauthorized) not
authorized on bandao_restore_drill_… to execute command { create: … }`, which
is the role restriction working, not a bug.

Point the drill at `root` through a throwaway env file. The script reads
`BANDAO_BACKUP_ENV` for exactly this. **A variable passed on the command line
will not work**: the script does `set -a; . "$ENV_FILE"; set +a`, so the file
is sourced *after* the environment and its `MONGO_URI` wins.

```bash
sudo mkdir -p /run/bandao-drill
sudo mount -t tmpfs -o size=64k,mode=0700 tmpfs /run/bandao-drill
```

Send the private key straight onto the tmpfs, never through a file on disk:

```bash
# from the operator's workstation, over Tailscale SSH
cat ~/path/to/bandao-backup.key | ssh root@<mongo-host> \
  'install -m 0400 /dev/stdin /run/bandao-drill/age.key'
```

Build the drill's env file from the real one with `MONGO_URI` swapped. The
password goes through a pipe and `printf` is a shell builtin, so it never
appears in `ps` output or shell history:

```bash
read -rs -p "root pw: " ROOT_PW; echo
{ sudo grep -v '^MONGO_URI=' /etc/bandao-backup.env
  printf 'MONGO_URI=mongodb://root:%s@127.0.0.1:27017/?authSource=admin\n' "$ROOT_PW"
} | sudo install -m 0600 /dev/stdin /run/bandao-drill/drill.env
unset ROOT_PW

sudo env \
  BANDAO_BACKUP_ENV=/run/bandao-drill/drill.env \
  AGE_IDENTITY_FILE=/run/bandao-drill/age.key \
  /usr/local/bin/bandao-restore-drill.sh

# Always tear down — this disposes of the key and the root credential together:
sudo umount /run/bandao-drill
sudo rmdir /run/bandao-drill
```

`ASSERT_COLLECTION` and `ASSERT_MIN_COUNT` are inherited from the copied file,
so they do not need passing separately.

Generate `root`'s password with `openssl rand -hex 32`. A `/` or `+` in it
must be percent-encoded here or `mongorestore` fails with `error parsing uri:
unescaped slash in password` — and, worse, a password that is *encoded when it
should not be* fails as a plain authentication error with nothing pointing at
the cause.

A passing run ends with:

```
32512 document(s) restored successfully. 0 document(s) failed to restore.
drill OK: daily/<stamp>.archive.gz.age restored, checkin_events count=11160
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
