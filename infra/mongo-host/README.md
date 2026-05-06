# Mongo host backup pipeline

Daily encrypted dump → S3 + monthly restore drill, packaged for the operator
to drop onto the production Mongo host.

## Files

| Path on repo | Path on host | Purpose |
| --- | --- | --- |
| `bandao-backup.sh` | `/usr/local/bin/bandao-backup.sh` | Daily dump + age + s3 cp |
| `bandao-restore-drill.sh` | `/usr/local/bin/bandao-restore-drill.sh` | Pulls latest dump, restores to scratch DB, asserts counts |
| `bandao-backup.service` | `/etc/systemd/system/bandao-backup.service` | One-shot unit invoked by the timer |
| `bandao-backup.timer` | `/etc/systemd/system/bandao-backup.timer` | Daily 03:30 schedule |

## One-time setup on the Mongo host

Assumes Debian 12 / Ubuntu 22.04+. Adjust package names for other distros.

```bash
# Tooling — mongo client, awscli v2, age.
sudo apt-get update
sudo apt-get install -y mongodb-mongosh mongodb-database-tools awscli age
```

Generate the encryption keypair on the operator's workstation (NOT on the
Mongo host) and copy only the public key to the host:

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
MONGO_URI=mongodb://backup_user:<pw>@127.0.0.1:27017/?authSource=admin
MONGO_DB=bandao
AGE_RECIPIENT=age1xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
S3_BUCKET=bandao-mongo-backups-apne1
S3_REGION=ap-northeast-1
S3_ACCESS_KEY_ID=AKIAxxxxxxxxxxxxxxxx
S3_SECRET_ACCESS_KEY=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
# optional:
# S3_PREFIX=bandao
```

Lock it down:

```bash
sudo chmod 0600 /etc/bandao-backup.env
sudo chown root:root /etc/bandao-backup.env
```

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
  ASSERT_MIN_COUNT=1 \
  /usr/local/bin/bandao-restore-drill.sh

# Always tear down:
sudo umount /run/bandao-drill
sudo rmdir /run/bandao-drill
```

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
