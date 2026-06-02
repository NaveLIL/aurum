pkgname=aurum
pkgver=0.2.0
pkgrel=1
desc="Terminal dashboard for Arch Linux / AUR with paru integration and PKGBUILD scanning"
arch=('x86_64')
url="https://github.com/NaveLIL/aurum"
license=('MIT')
depends=('paru' 'libgit2')
optdepends=('flatpak: Flatpak search, installed apps view, and app install/uninstall support')
makedepends=('cargo' 'rust' 'pkg-config' 'libgit2')
source=("aurum::https://github.com/NaveLIL/aurum/archive/18f8a4ee8d7a6055b24f7c7d3e77993b20f87a1e.tar.gz")
sha256sums=('d5558cd419c8d46bdc958064cb97f963d1ea793866414c025906ec15033512ed')

build() {
  cd "${srcdir}/aurum-18f8a4ee8d7a6055b24f7c7d3e77993b20f87a1e"
  cargo build --release --locked
}

package() {
  cd "${srcdir}/aurum-18f8a4ee8d7a6055b24f7c7d3e77993b20f87a1e"
  install -Dm755 "target/release/aurum" "${pkgdir}/usr/bin/aurum"
  install -Dm644 "aurum.desktop" "${pkgdir}/usr/share/applications/aurum.desktop"
}
