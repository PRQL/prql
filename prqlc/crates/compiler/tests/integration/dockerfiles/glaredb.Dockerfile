# syntax=docker/dockerfile:1.4

# TODO: switch to `curlimages/curl`
# glaredb binary releases are not compatible with alpine
# So we can't use here now
# https://github.com/GlareDB/glaredb/issues/1912
FROM --platform=linux/amd64 docker.io/library/buildpack-deps:stable

RUN <<EOF
cd /usr/local/bin
curl https://glaredb.com/install.sh | sh
EOF

CMD [ "glaredb", "server" ]
