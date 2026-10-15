# Where published builds go and what they connect to; read by scripts/release.sh
# (docs/RELEASES.md, "Publishing"). Nothing secret lives here: the R2 keys are in
# the AWS CLI profile, the release key in ~/.config/meridian-release/.

# The public URL players download builds from: the R2 bucket's public domain.
STORE_URL="https://builds.meridianconflict.com"
# The R2 bucket and its S3 endpoint (https://<account id>.r2.cloudflarestorage.com).
R2_BUCKET="meridian-builds"
R2_ENDPOINT="https://674c2382543440ce4fd497aafd05db48.r2.cloudflarestorage.com"
# The AWS CLI profile holding the bucket's R2 access key (aws configure --profile ...).
R2_PROFILE="meridian-r2"

# Each channel's multiplayer server, host[:port] (docs/SERVER.md).
# playtest.meridianconflict.com is a DNS-only CNAME to the Railway relay's TCP proxy
# (shortline.proxy.rlwy.net, internal port 7777): moving the relay is a DNS change,
# not a new build. The release channel has no server yet.
SERVER_PLAYTEST="playtest.meridianconflict.com:27824"
SERVER_RELEASE=""
