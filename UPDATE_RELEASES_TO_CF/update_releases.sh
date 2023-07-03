set -e

gh release download \
    --pattern '*.AppImage' --pattern '*.msi' --pattern '*.dmg' \
    --dir downloads

