# Where published builds go and what they connect to; read by scripts/release.sh
# (docs/RELEASES.md, "Publishing"). Nothing secret lives here: the R2 keys are in
# the AWS CLI profile, the release key in ~/.config/meridian-release/.

# The public URL players download builds from: the R2 bucket's public domain.
STORE_URL="file:///C:/Users/joshu/AppData/Local/mc-release-test/store"
# The R2 bucket and its S3 endpoint (https://<account id>.r2.cloudflarestorage.com).
R2_BUCKET=""
R2_ENDPOINT=""
# The AWS CLI profile holding the bucket's R2 access key (aws configure --profile ...).
R2_PROFILE="meridian-r2"

# Each channel's multiplayer server, host[:port] (docs/SERVER.md).
SERVER_PLAYTEST=""
SERVER_RELEASE=""
