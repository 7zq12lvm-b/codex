compile() {
    VERSION=$1
    echo "Compiling codex-cli..."
    cd codex-rs && cargo build -p codex-cli --release
}

compress() {
    VERSION=$1
    echo "Compressing codex-cli..."
    cp codex-rs/target/release/codex /tmp/codex-cli
    cp config.toml.example /tmp/config.toml
    tar -czvf /tmp/codex-${VERSION}.tar.gz -C /tmp codex-cli config.toml
    rm -f /tmp/codex-cli /tmp/config.toml
}

upload() {
    VERSION=$1
    echo "Uploading codex-cli to OSS..."
    ossutil cp /tmp/codex-${VERSION}.tar.gz oss://lsh-oss-it-log/codex-${VERSION}.tar.gz
    ossutil set-acl oss://lsh-oss-it-log/codex-${VERSION}.tar.gz public-read
}

install() {
    VERSION=$1
    echo "Downloading codex-cli from OSS..."
    wget https://lsh-oss-it-log.oss-cn-shanghai.aliyuncs.com/codex-${VERSION}.tar.gz -O /tmp/codex-${VERSION}.tar.gz
    sudo mkdir -p /usr/local/bin
    sudo tar -xzvf /tmp/codex-${VERSION}.tar.gz -C /tmp/codex-extract
    sudo cp /tmp/codex-extract/codex-cli /usr/local/bin/
    sudo chmod +x /usr/local/bin/codex-cli
    mkdir -p ~/.config/codex-cli
    cp /tmp/codex-extract/config.toml ~/.config/codex-cli/config.toml
    rm -rf /tmp/codex-extract
}



# check parameters 
if [ $# -ne 2 ]; then
    echo "Usage: $0 {compile|compress|upload|install|full} <version>"
    exit 1
fi

case "$1" in
    compile)
        compile $2
        ;;
    compress)
        compress $2
        ;;
    upload)
        upload $2
        ;;
    install)
        install $2
        ;;
    full)
        compile $2
        compress $2
        upload $2
        install $2
        ;;
    *)
        echo "Usage: $0 {compile|compress|upload|install} <version>"
        exit 1
esac