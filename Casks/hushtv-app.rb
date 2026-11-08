# Written by scripts/packages.py on every release. Don't edit by hand.
cask "hushtv-app" do
  version "0.2.0-beta.6"
  sha256 "fe7a122d4f359ed5fe9aec2a3b591d89140adc63e4174297a6a6e11a7f891807"

  url "https://github.com/kajisho5/hushtv/releases/download/v#{version}/hushtv_#{version}_macos-universal.dmg"
  name "hushtv"
  desc "Switch off ads, trackers and promo apps on Android TV and Google TV - undoably"
  homepage "https://github.com/kajisho5/hushtv"

  depends_on macos: ">= :high_sierra"

  app "hushtv.app"

  zap trash: [
    "~/Library/Application Support/io.github.kajisho5.hushtv",
    "~/Library/Caches/io.github.kajisho5.hushtv",
    "~/Library/WebKit/io.github.kajisho5.hushtv",
  ]
end
