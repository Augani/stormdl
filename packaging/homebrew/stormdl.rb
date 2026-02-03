class Stormdl < Formula
  desc "Next-generation download accelerator with adaptive multi-segment parallel downloads"
  homepage "https://github.com/augustusotu/stormdl"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/augustusotu/stormdl/releases/download/v#{version}/storm-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_ARM64_MACOS"
    end
    on_intel do
      url "https://github.com/augustusotu/stormdl/releases/download/v#{version}/storm-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_X64_MACOS"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/augustusotu/stormdl/releases/download/v#{version}/storm-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "PLACEHOLDER_SHA256_ARM64_LINUX"
    end
    on_intel do
      url "https://github.com/augustusotu/stormdl/releases/download/v#{version}/storm-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "PLACEHOLDER_SHA256_X64_LINUX"
    end
  end

  def install
    bin.install "storm"
  end

  test do
    system "#{bin}/storm", "--version"
  end
end
