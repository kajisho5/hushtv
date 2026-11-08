# Written by scripts/packages.py on every release. Don't edit by hand.
class Hushtv < Formula
  desc "Switch off ads, trackers and promo apps on Android TV and Google TV - undoably"
  homepage "https://github.com/kajisho5/hushtv"
  version "0.2.0-beta.6"
  license "MIT"

  on_macos do
    url "https://github.com/kajisho5/hushtv/releases/download/v0.2.0-beta.6/hushtv-macos-universal.zip"
    sha256 "d5d62f2b1d6044c53ba0b4ef11d8a4515255fa920468e43def4f10b89286d793"
  end

  on_linux do
    on_intel do
      url "https://github.com/kajisho5/hushtv/releases/download/v0.2.0-beta.6/hushtv-linux-x86_64.tar.gz"
      sha256 "f86d180333f0b26ba53244d63bcc999bae5d41f1290f487a795133e7231badc0"
    end
    on_arm do
      url "https://github.com/kajisho5/hushtv/releases/download/v0.2.0-beta.6/hushtv-linux-arm64.tar.gz"
      sha256 "beadb451c066ff41ef2d62858b11bd0a96912bd984ee080bbb498003a66e9ab8"
    end
  end

  def install
    bin.install "hushtv"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/hushtv --version")
  end
end
