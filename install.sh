#!/bin/bash
set -e

echo "🔨 Building Aurum (release)..."
cargo build --release

echo "📦 Installing binary to ~/.local/bin..."
mkdir -p ~/.local/bin
rm -f ~/.local/bin/aurum
cp target/release/aurum ~/.local/bin/aurum
chmod +x ~/.local/bin/aurum

echo "🖥 Installing desktop launcher..."
mkdir -p ~/.local/share/applications
cp aurum.desktop ~/.local/share/applications/aurum.desktop

echo ""
echo "✅ Done! You can now:"
echo "   • Run 'aurum' from terminal"
echo "   • Find 'Aurum' in your application menu"
echo ""
echo "Make sure ~/.local/bin is in your PATH."
