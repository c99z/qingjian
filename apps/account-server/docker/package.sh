#!/usr/bin/env bash
# 打离线部署包：账号服务与 Caddy 镜像 + compose.yaml + Caddyfile + env.example，拷到目标服务器解包即可起服务，
# 目标机不需要源码、Rust 工具链或访问 crates.io。
#   apps/account-server/docker/package.sh [输出目录]     默认 target/account-server-docker
# 目标机是 ARM 时先 export DOCKER_DEFAULT_PLATFORM=linux/arm64（本机需装 binfmt/QEMU）。
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
out="${1:-$root/target/account-server-docker}"

version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/apps/account-server/Cargo.toml" | head -n1)"
if [[ "$version" == *-dev ]]; then
  version="$version-$(git -C "$root" rev-parse --short HEAD)"
  # Docker 标签不能带 +，脏工作区用 -dirty
  git -C "$root" diff --quiet HEAD -- || version="$version-dirty"
fi
image="qingjian-account-server:$version"
name="qingjian-account-server-$version-docker"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

echo "构建镜像 $image"
QINGJIAN_ACCOUNT_IMAGE="$image" QINGJIAN_SITE=build-only \
  docker compose --project-directory "$here" -f "$here/compose.yaml" build account

# Caddy 镜像一起打进去：目标机常常访问不了 Docker Hub
caddy="${QINGJIAN_CADDY_IMAGE:-caddy:2.10-alpine}"
docker image inspect "$caddy" >/dev/null 2>&1 || docker pull "$caddy"

mkdir -p "$stage/$name"
docker save "$image" "$caddy" -o "$stage/$name/image.tar"
# 部署包里没有源码，去掉 build 段，compose 只用已导入的镜像
sed '/^    build:$/,/^    image:/{/^    image:/!d}' "$here/compose.yaml" > "$stage/$name/compose.yaml"
cp "$here/Caddyfile" "$stage/$name/"
sed -e "s|^# QINGJIAN_ACCOUNT_IMAGE=.*|QINGJIAN_ACCOUNT_IMAGE=$image|" \
    -e "s|^# QINGJIAN_CADDY_IMAGE=.*|QINGJIAN_CADDY_IMAGE=$caddy|" \
    "$here/env.example" > "$stage/$name/env.example"
cat > "$stage/$name/README.txt" <<TXT
青简账号服务 $version 离线部署包

  docker load -i image.tar
  cp env.example .env        # 填 QINGJIAN_SITE（服务器 IP 或域名）等
  docker compose up -d
  docker compose ps          # account 与 caddy 均为 healthy / running

使用自建 CA（QINGJIAN_TLS=internal）时导出根证书，装到每台客户端：
  docker compose exec caddy cat /data/caddy/pki/authorities/local/root.crt > qingjian-account-ca.crt

完整说明见源码仓库 apps/account-server/README.md。
TXT

mkdir -p "$out"
tar -C "$stage" -czf "$out/$name.tar.gz" "$name"
echo "已生成 $out/$name.tar.gz"
