install() {
    VERSION=0.120.0-3
    echo "Downloading codex-cli from OSS..."
    curl -fSL "https://lsh-oss-it-log.oss-cn-shanghai.aliyuncs.com/codex-${VERSION}.tar.gz" -o "/tmp/codex-${VERSION}.tar.gz"
    mkdir -p /tmp/codex-extract
    tar -xzvf "/tmp/codex-${VERSION}.tar.gz" -C /tmp/codex-extract
    sudo mkdir -p /usr/local/bin
    sudo cp /tmp/codex-extract/codex-cli /usr/local/bin/
    sudo chmod +x /usr/local/bin/codex-cli
    mkdir -p ~/.codex
    cp /tmp/codex-extract/config.toml ~/.codex/config.toml
    rm -rf /tmp/codex-extract
}

install