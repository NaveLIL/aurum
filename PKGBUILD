pkgname=aurum
pkgver=0.1.0
pkgrel=1
desc="Terminal dashboard for Arch Linux / AUR with paru integration and PKGBUILD scanning"
arch=('x86_64')
url="https://github.com/NaveLIL/aurum"
license=('MIT')
depends=('paru' 'libgit2')
makedepends=('cargo' 'rust' 'pkg-config' 'libgit2')
source=("aurum::https://github.com/NaveLIL/aurum/archive/825cb51eb891548b80812028c9f98944e6d5fddb.tar.gz")
sha256sums=('e36e1a37291b49456ca45fbe519cb0c23de7e423b81975e3aa896769b1be6173')

build() {
  cd "${srcdir}/aurum-825cb51eb891548b80812028c9f98944e6d5fddb"
  cargo build --release --locked
}

package() {
  cd "${srcdir}/aurum-825cb51eb891548b80812028c9f98944e6d5fddb"
  install -Dm755 "target/release/aurum" "${pkgdir}/usr/bin/aurum"
  install -Dm644 "aurum.desktop" "${pkgdir}/usr/share/applications/aurum.desktop"
}
